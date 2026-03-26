import type { Context } from "hono";
import { ilike, or, sql } from "drizzle-orm";
import { db, memory } from "@nova/db";
import { generateEmbedding } from "../embedding.js";
import type { MemorySvcConfig } from "../config.js";
import type { Logger } from "../logger.js";

const DEFAULT_LIMIT = 10;
const MAX_LIMIT = 50;

export async function handleSearch(
  c: Context,
  config: MemorySvcConfig,
  logger: Logger,
) {
  const body = await c.req.json<{ query?: string; limit?: number }>();
  const { query, limit: rawLimit } = body;

  if (!query || typeof query !== "string") {
    return c.json({ error: "query is required" }, 400);
  }

  const limit = Math.min(
    Math.max(1, rawLimit ?? DEFAULT_LIMIT),
    MAX_LIMIT,
  );

  // Try vector search first
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

    return c.json({
      results: results.map((r) => ({
        topic: r.topic,
        content: r.content,
        similarity: r.similarity,
        updatedAt: r.updatedAt.toISOString(),
      })),
    });
  }

  // Fallback: case-insensitive substring search on topic + content
  logger.info("Using substring search fallback (no embeddings available)");
  const pattern = `%${query}%`;
  const results = await db
    .select({
      topic: memory.topic,
      content: memory.content,
      updatedAt: memory.updatedAt,
    })
    .from(memory)
    .where(
      or(
        ilike(memory.topic, pattern),
        ilike(memory.content, pattern),
      ),
    )
    .limit(limit);

  return c.json({
    results: results.map((r) => ({
      topic: r.topic,
      content: r.content,
      similarity: null,
      updatedAt: r.updatedAt.toISOString(),
    })),
  });
}
