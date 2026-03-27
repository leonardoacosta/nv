import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import type { DiscordClient } from "./client.js";
import type { Logger } from "./logger.js";
import { listGuilds } from "./tools/guilds.js";
import { listChannels } from "./tools/channels.js";
import { readMessages } from "./tools/messages.js";

export async function startMcpServer(
  client: DiscordClient,
  logger: Logger,
): Promise<void> {
  const server = new McpServer({
    name: "discord-svc",
    version: "0.1.0",
  });

  // discord_list_guilds — no params
  server.registerTool(
    "discord_list_guilds",
    {
      description:
        "List Discord servers (guilds) the bot is a member of. Returns server names and IDs.",
      inputSchema: z.object({}),
    },
    async () => {
      const result = await listGuilds(client);
      return {
        content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }],
      };
    },
  );

  // discord_list_channels — requires guild_id
  server.registerTool(
    "discord_list_channels",
    {
      description:
        "List text channels in a Discord server by guild ID. Returns channel names, IDs, and topics.",
      inputSchema: z.object({
        guild_id: z
          .string()
          .describe(
            "Discord guild (server) ID. Use discord_list_guilds to find available IDs.",
          ),
      }),
    },
    async ({ guild_id }) => {
      const result = await listChannels(client, guild_id);
      return {
        content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }],
      };
    },
  );

  // discord_read_messages — requires channel_id, optional limit
  server.registerTool(
    "discord_read_messages",
    {
      description:
        "Read recent messages from a Discord channel. Returns author, timestamp, and content.",
      inputSchema: z.object({
        channel_id: z
          .string()
          .describe(
            "Discord channel ID. Use discord_list_channels to find available channel IDs.",
          ),
        limit: z
          .number()
          .optional()
          .describe("Maximum number of messages to return (default 50, max 100)."),
      }),
    },
    async ({ channel_id, limit }) => {
      const result = await readMessages(client, channel_id, limit ?? 50);
      return {
        content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }],
      };
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);

  logger.info(
    { service: "discord-svc", transport: "mcp" },
    "MCP stdio server started",
  );
}
