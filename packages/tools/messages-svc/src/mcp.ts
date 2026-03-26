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
        "Retrieve the most recent messages, optionally filtered by channel. Returns messages ordered by created_at descending.",
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
        "Search messages using ILIKE text matching on content. Optionally filter by channel.",
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
