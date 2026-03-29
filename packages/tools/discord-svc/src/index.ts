import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";

import { DiscordClient, DiscordApiError } from "./client.js";
import { createLogger } from "./logger.js";
import { startMcpServer } from "./mcp.js";
import { listGuilds } from "./tools/guilds.js";
import { listChannels } from "./tools/channels.js";
import { readMessages } from "./tools/messages.js";

const SERVICE_NAME = "discord-svc";
const DEFAULT_PORT = 4104;

// Require bot token
const botToken = process.env["DISCORD_BOT_TOKEN"];
if (!botToken) {
  process.stderr.write("DISCORD_BOT_TOKEN not set — exiting\n");
  process.exit(1);
}

const isMcpMode = process.argv.includes("--mcp");
const servicePort = parseInt(process.env["PORT"] ?? String(DEFAULT_PORT), 10);
const corsOrigin =
  process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev";

// In MCP mode, logger must write to stderr to avoid corrupting stdio protocol
const logger = createLogger(SERVICE_NAME, {
  ...(isMcpMode ? { destination: process.stderr } : {}),
});

const client = new DiscordClient(botToken);

if (isMcpMode) {
  // MCP stdio transport
  await startMcpServer(client, logger);
} else {
  // HTTP transport
  const startedAt = Date.now();
  const app = new Hono();

  // Middleware
  app.use("*", cors({ origin: corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    logger.error({ err }, "Unhandled error");
    if (err instanceof DiscordApiError) {
      return c.json({ error: err.message }, err.status as 401 | 403 | 404 | 500);
    }
    return c.json(
      { error: err instanceof Error ? err.message : "Internal Server Error" },
      500,
    );
  });

  // Health endpoint
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: SERVICE_NAME,
      port: servicePort,
    });
  });

  // Registry endpoint — exposes tool definitions for tool-router self-registration
  app.get("/registry", (c) => {
    return c.json({
      service: "discord-svc",
      tools: [
        {
          name: "discord_list_guilds",
          description: "List all Discord guilds (servers) the bot is a member of.",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
          name: "discord_list_channels",
          description: "List text channels in a Discord guild.",
          inputSchema: {
            type: "object",
            properties: {
              guild_id: { type: "string", description: "Discord guild (server) ID" },
            },
            required: ["guild_id"],
          },
        },
        {
          name: "discord_read_messages",
          description: "Read recent messages from a Discord channel.",
          inputSchema: {
            type: "object",
            properties: {
              channel_id: { type: "string", description: "Discord channel ID" },
              limit: { type: "number", description: "Number of messages to return (max: 100)" },
            },
            required: ["channel_id"],
          },
        },
      ],
      healthUrl: `http://127.0.0.1:${servicePort}/health`,
    });
  });

  // GET /guilds — list all guilds the bot is in
  app.get("/guilds", async (c) => {
    try {
      const result = await listGuilds(client);
      return c.json(result);
    } catch (err) {
      if (err instanceof DiscordApiError) {
        return c.json({ error: err.message }, err.status as 401 | 403 | 500);
      }
      throw err;
    }
  });

  // GET /channels/:guildId — list text channels in a guild
  app.get("/channels/:guildId", async (c) => {
    const guildId = c.req.param("guildId");
    try {
      const result = await listChannels(client, guildId);
      return c.json(result);
    } catch (err) {
      if (err instanceof DiscordApiError) {
        return c.json({ error: err.message }, err.status as 401 | 403 | 404 | 500);
      }
      throw err;
    }
  });

  // GET /messages/:channelId — read messages from a channel
  app.get("/messages/:channelId", async (c) => {
    const channelId = c.req.param("channelId");
    const limitParam = c.req.query("limit");
    const limit = limitParam ? parseInt(limitParam, 10) : 50;
    const clampedLimit = Math.max(1, Math.min(100, isNaN(limit) ? 50 : limit));

    try {
      const result = await readMessages(client, channelId, clampedLimit);
      return c.json(result);
    } catch (err) {
      if (err instanceof DiscordApiError) {
        return c.json({ error: err.message }, err.status as 401 | 403 | 404 | 500);
      }
      throw err;
    }
  });

  const server = serve(
    { fetch: app.fetch, port: servicePort },
    (info) => {
      logger.info(
        {
          service: SERVICE_NAME,
          port: info.port,
          transport: "http",
        },
        `${SERVICE_NAME} listening on :${info.port}`,
      );
    },
  );

  // Graceful shutdown
  const shutdown = () => {
    logger.info("Shutting down...");
    server.close(() => {
      logger.info("Server closed");
      process.exit(0);
    });
  };

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);
}
