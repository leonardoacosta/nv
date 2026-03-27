import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import type { ServiceConfig } from "./config.js";
import type { Logger } from "./logger.js";
import { getRecentMessages, searchMessages } from "./tools.js";

export async function startMcpServer(
  config: ServiceConfig,
  logger: Logger,
): Promise<void> {
  const server = new McpServer({
    name: config.serviceName,
    version: "0.1.0",
  });

  // get_recent_messages
  server.registerTool(
    "get_recent_messages",
    {
      description:
        "Get recent conversation messages, optionally filtered by channel. Returns sender, content, and timestamp.",
      inputSchema: z.object({
        channel: z
          .string()
          .optional()
          .describe("Filter to a specific channel (telegram, discord, teams, etc.)"),
        limit: z
          .number()
          .int()
          .min(1)
          .max(100)
          .optional()
          .describe("Number of messages to return (default: 20, max: 100)"),
      }),
    },
    async ({ channel, limit }) => {
      const results = await getRecentMessages(channel, limit);
      return {
        content: [{ type: "text" as const, text: JSON.stringify(results) }],
      };
    },
  );

  // search_messages
  server.registerTool(
    "search_messages",
    {
      description:
        "Full-text search across all stored messages. Filter by channel and limit results.",
      inputSchema: z.object({
        query: z.string().min(1).describe("Search query text"),
        channel: z
          .string()
          .optional()
          .describe("Filter results to a specific channel"),
        limit: z
          .number()
          .int()
          .min(1)
          .max(50)
          .optional()
          .describe("Max results to return (default: 10, max: 50)"),
      }),
    },
    async ({ query, channel, limit }) => {
      const results = await searchMessages(query, channel, limit);
      return {
        content: [{ type: "text" as const, text: JSON.stringify(results) }],
      };
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);

  logger.info(
    { service: config.serviceName, transport: "mcp" },
    "MCP stdio server started",
  );
}
