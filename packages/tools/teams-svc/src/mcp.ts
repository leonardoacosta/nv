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
        "List Microsoft Teams teams and chats. " +
        "Returns teams, DMs, and group chats accessible from the CloudPC account. " +
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
        "Read recent messages from a Microsoft Teams chat (DM or group chat). " +
        "Returns sender, timestamp, and content. " +
        "Use teams_list_chats to find chat IDs.",
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
        "Read recent messages from a Microsoft Teams channel. " +
        "Returns messages with sender and timestamp. " +
        "Specify channel_name to read a specific channel; omit to read General.",
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
        "List channels in a Microsoft Teams team. Returns channel names. " +
        "Uses team name (display name), not team ID.",
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
        "Check a Microsoft Teams user's presence and availability status. " +
        "Returns availability (Available, Busy, DoNotDisturb, Away, Offline) and " +
        "activity (InACall, InAMeeting, Presenting, etc.).",
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
        "Send a message to a Microsoft Teams chat. " +
        "Requires explicit user confirmation before sending. " +
        "Use teams_list_chats to find chat IDs.",
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
