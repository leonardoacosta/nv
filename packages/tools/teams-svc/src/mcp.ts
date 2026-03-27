import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import type { Logger } from "./logger.js";
import {
  teamsListChats,
  teamsReadChat,
  teamsMessages,
  teamsChannels,
  teamsPresence,
  teamsSend,
} from "./tools/index.js";

export async function startMcpServer(logger: Logger): Promise<void> {
  const server = new McpServer({
    name: "teams-svc",
    version: "0.1.0",
  });

  // teams_list_chats
  server.registerTool(
    "teams_list_chats",
    {
      description:
        "List your recent Microsoft Teams chats and DMs. Authenticated and ready to use. " +
        "Use the returned chat ID with teams_read_chat to read messages.",
      inputSchema: z.object({
        limit: z
          .number()
          .min(1)
          .max(50)
          .optional()
          .describe("Maximum number of chats to return (default 20, max 50)"),
      }),
    },
    async ({ limit }) => {
      const result = await teamsListChats(limit);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  // teams_read_chat
  server.registerTool(
    "teams_read_chat",
    {
      description:
        "Read recent messages from a Teams chat by chat ID. Returns sender, timestamp, and content.",
      inputSchema: z.object({
        chat_id: z
          .string()
          .describe(
            "Teams chat ID (e.g. '19:...'). Use teams_list_chats to find available chat IDs.",
          ),
        limit: z
          .number()
          .min(1)
          .max(50)
          .optional()
          .describe("Maximum number of messages to return (default 20, max 50)"),
      }),
    },
    async ({ chat_id, limit }) => {
      const result = await teamsReadChat(chat_id, limit);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  // teams_messages
  server.registerTool(
    "teams_messages",
    {
      description:
        "Read recent messages from a Teams channel. Provide team and channel names. " +
        "Defaults to the General channel if channel_name is omitted.",
      inputSchema: z.object({
        team_name: z
          .string()
          .describe("Teams team display name (e.g. 'WholesaleIT'). Required."),
        channel_name: z
          .string()
          .optional()
          .describe(
            "Channel display name (e.g. 'Dev'). Defaults to General if omitted.",
          ),
        count: z
          .number()
          .optional()
          .describe("Number of messages to return (default 20)."),
      }),
    },
    async ({ team_name, channel_name, count }) => {
      const result = await teamsMessages(team_name, channel_name, count);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  // teams_channels
  server.registerTool(
    "teams_channels",
    {
      description:
        "List channels in a Teams team. Returns channel names and IDs.",
      inputSchema: z.object({
        team_name: z
          .string()
          .describe("Teams team display name (e.g. 'WholesaleIT'). Required."),
      }),
    },
    async ({ team_name }) => {
      const result = await teamsChannels(team_name);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  // teams_presence
  server.registerTool(
    "teams_presence",
    {
      description:
        "Check a Teams user's presence/availability status by email or UPN. " +
        "Returns availability (Available, Busy, Away, Offline, etc.) and activity.",
      inputSchema: z.object({
        user: z
          .string()
          .describe(
            "User email/UPN (e.g. sarah@civalent.com) or Azure AD object ID.",
          ),
      }),
    },
    async ({ user }) => {
      const result = await teamsPresence(user);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  // teams_send
  server.registerTool(
    "teams_send",
    {
      description:
        "Send a message to a Teams chat. Requires operator confirmation before sending.",
      inputSchema: z.object({
        chat_id: z
          .string()
          .describe("Teams chat ID. Use teams_list_chats to find available chat IDs."),
        message: z
          .string()
          .describe("Message content to send (plain text)."),
      }),
    },
    async ({ chat_id, message }) => {
      const result = await teamsSend(chat_id, message);
      return { content: [{ type: "text" as const, text: result }] };
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);

  logger.info(
    { service: "teams-svc", transport: "mcp" },
    "MCP stdio server started",
  );
}
