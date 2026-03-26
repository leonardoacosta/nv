import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { eq, ilike, or, sql } from "drizzle-orm";
import { db, memory } from "@nova/db";

import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { generateEmbedding } from "./embedding.js";
import { writeMemoryFile, readMemoryFile } from "./filesystem.js";

const config = loadConfig();
const logger = createLogger(config.serviceName, { destination: process.stderr });

const MAX_MEMORY_READ_CHARS = 20_000;
const DEFAULT_SEARCH_LIMIT = 10;
const MAX_SEARCH_LIMIT = 50;

function truncate(content: string): string {
  if (content.length <= MAX_MEMORY_READ_CHARS) return content;
  return content.slice(0, MAX_MEMORY_READ_CHARS);
}

const server = new McpServer({
  name: "memory-svc",
  version: "0.1.0",
});

// read_memory tool
server.registerTool(
  "read_memory",
  {
    description: "Read a memory topic by name. Returns the content and last updated timestamp.",
    inputSchema: z.object({
      topic: z.string().describe("The memory topic to read"),
    }),
  },
  async ({ topic }) => {
    const row = await db.query.memory.findFirst({
      where: eq(memory.topic, topic),
    });

    if (row) {
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify({
              topic: row.topic,
              content: truncate(row.content),
              updatedAt: row.updatedAt.toISOString(),
            }),
          },
        ],
      };
    }

    // Fallback to filesystem
    const fileResult = await readMemoryFile(config.memoryDir, topic);
    if (fileResult) {
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify({
              topic,
              content: truncate(fileResult.content),
              updatedAt: fileResult.updatedAt.toISOString(),
            }),
          },
        ],
      };
    }

    return {
      content: [{ type: "text" as const, text: JSON.stringify({ error: "not_found" }) }],
      isError: true,
    };
  },
);

// write_memory tool
server.registerTool(
  "write_memory",
  {
    description: "Write or update a memory topic. Upserts into Postgres and syncs to filesystem.",
    inputSchema: z.object({
      topic: z.string().describe("The memory topic name"),
      content: z.string().describe("The memory content to store"),
    }),
  },
  async ({ topic, content }) => {
    const existing = await db.query.memory.findFirst({
      where: eq(memory.topic, topic),
      columns: { id: true },
    });

    const embedding = await generateEmbedding(
      `${topic}\n\n${content}`,
      config.openaiApiKey,
      logger,
    );

    await db
      .insert(memory)
      .values({
        topic,
        content,
        embedding: embedding ?? undefined,
        updatedAt: new Date(),
      })
      .onConflictDoUpdate({
        target: memory.topic,
        set: {
          content,
          embedding: embedding ?? sql`memory.embedding`,
          updatedAt: new Date(),
        },
      });

    const action = existing ? "updated" : "created";

    try {
      await writeMemoryFile(config.memoryDir, topic, content);
    } catch (err) {
      logger.error({ err, topic }, "Failed to sync memory to filesystem");
    }

    return {
      content: [{ type: "text" as const, text: JSON.stringify({ topic, action }) }],
    };
  },
);

// search_memory tool
server.registerTool(
  "search_memory",
  {
    description: "Search memory by semantic similarity (pgvector) or substring match as fallback.",
    inputSchema: z.object({
      query: z.string().describe("Search query string"),
      limit: z.number().min(1).max(MAX_SEARCH_LIMIT).optional()
        .describe(`Max results to return (default: ${DEFAULT_SEARCH_LIMIT}, max: ${MAX_SEARCH_LIMIT})`),
    }),
  },
  async ({ query, limit: rawLimit }) => {
    const limit = Math.min(Math.max(1, rawLimit ?? DEFAULT_SEARCH_LIMIT), MAX_SEARCH_LIMIT);

    const queryEmbedding = await generateEmbedding(query, config.openaiApiKey, logger);

    if (queryEmbedding) {
      const vectorStr = `[${queryEmbedding.join(",")}]`;
      const results = await db
        .select({
          topic: memory.topic,
          content: memory.content,
          similarity: sql<number>`1 - (${memory.embedding} <=> ${vectorStr}::vector)`.as("similarity"),
          updatedAt: memory.updatedAt,
        })
        .from(memory)
        .where(sql`${memory.embedding} IS NOT NULL`)
        .orderBy(sql`${memory.embedding} <=> ${vectorStr}::vector`)
        .limit(limit);

      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify({
              results: results.map((r) => ({
                topic: r.topic,
                content: r.content,
                similarity: r.similarity,
                updatedAt: r.updatedAt.toISOString(),
              })),
            }),
          },
        ],
      };
    }

    // Fallback: substring search
    logger.info("Using substring search fallback (no embeddings available)");
    const pattern = `%${query}%`;
    const results = await db
      .select({
        topic: memory.topic,
        content: memory.content,
        updatedAt: memory.updatedAt,
      })
      .from(memory)
      .where(or(ilike(memory.topic, pattern), ilike(memory.content, pattern)))
      .limit(limit);

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify({
            results: results.map((r) => ({
              topic: r.topic,
              content: r.content,
              similarity: null,
              updatedAt: r.updatedAt.toISOString(),
            })),
          }),
        },
      ],
    };
  },
);

// Start MCP stdio transport
const transport = new StdioServerTransport();
await server.connect(transport);
logger.info({ service: config.serviceName, transport: "mcp" }, "MCP stdio server started");
