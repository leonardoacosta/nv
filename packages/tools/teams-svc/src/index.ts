import { serve } from "@hono/node-server";
import { Hono, type Context } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";

import { createLogger } from "./logger.js";
import { startMcpServer } from "./mcp.js";
import { CloudPcUnreachableError, CloudPcScriptError } from "./ssh.js";
import {
  teamsListChats,
  teamsReadChat,
  teamsMessages,
  teamsChannels,
  teamsPresence,
  teamsSend,
} from "./tools/index.js";

const SERVICE_NAME = "teams-svc";
const DEFAULT_PORT = 4005;

const isMcpMode = process.argv.includes("--mcp");
const servicePort = parseInt(process.env["PORT"] ?? String(DEFAULT_PORT), 10);
const corsOrigin =
  process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev";

// In MCP mode, logger must write to stderr to avoid corrupting stdio protocol
const logger = createLogger(SERVICE_NAME, {
  ...(isMcpMode ? { destination: process.stderr } : {}),
});

/**
 * Map tool errors to appropriate HTTP status codes.
 */
function handleToolError(c: Context, err: unknown) {
  if (err instanceof CloudPcUnreachableError) {
    logger.error({ err }, "CloudPC unreachable");
    return c.json({ ok: false, error: err.message }, 503);
  }
  if (err instanceof CloudPcScriptError) {
    logger.error({ err }, "CloudPC script error");
    return c.json({ ok: false, error: err.message }, 502);
  }
  if (err instanceof Error && err.message.includes("is required")) {
    return c.json({ ok: false, error: err.message }, 400);
  }
  const message = err instanceof Error ? err.message : "Internal Server Error";
  logger.error({ err }, "Unexpected error");
  return c.json({ ok: false, error: message }, 500);
}

interface SearchBody {
  team_name?: string;
  channel_name?: string;
  count?: number;
}

interface SendBody {
  chat_id?: string;
  message?: string;
}

if (isMcpMode) {
  await startMcpServer(logger);
} else {
  const app = new Hono();
  const startedAt = Date.now();

  // Middleware
  app.use("*", cors({ origin: corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    logger.error({ err }, "Unhandled error");

    if (err instanceof CloudPcUnreachableError) {
      return c.json({ ok: false, error: err.message }, 503);
    }
    if (err instanceof CloudPcScriptError) {
      return c.json({ ok: false, error: err.message }, 502);
    }

    return c.json(
      { ok: false, error: err instanceof Error ? err.message : "Internal Server Error" },
      500,
    );
  });

  // Health endpoint
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: SERVICE_NAME,
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
    });
  });

  // GET /chats -- list recent chats
  app.get("/chats", async (c) => {
    const limitParam = c.req.query("limit");
    const limit = limitParam ? parseInt(limitParam, 10) : undefined;

    try {
      const data = await teamsListChats(limit);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
    }
  });

  // GET /chats/:id -- read messages from a specific chat
  app.get("/chats/:id", async (c) => {
    const chatId = c.req.param("id");
    const limitParam = c.req.query("limit");
    const limit = limitParam ? parseInt(limitParam, 10) : undefined;

    try {
      const data = await teamsReadChat(chatId, limit);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
    }
  });

  // GET /channels -- list channels in a team
  app.get("/channels", async (c) => {
    const teamName = c.req.query("team_name");
    if (!teamName?.trim()) {
      return c.json({ ok: false, error: "team_name query parameter is required" }, 400);
    }

    try {
      const data = await teamsChannels(teamName);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
    }
  });

  // POST /search -- read messages from a channel
  app.post("/search", async (c) => {
    let body: SearchBody = {};
    try {
      body = await c.req.json<SearchBody>();
    } catch {
      // Empty or malformed body
    }

    const teamName = body.team_name;
    if (!teamName?.trim()) {
      return c.json({ ok: false, error: "team_name is required in request body" }, 400);
    }

    try {
      const data = await teamsMessages(teamName, body.channel_name, body.count);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
    }
  });

  // GET /presence -- check user presence
  app.get("/presence", async (c) => {
    const user = c.req.query("user");
    if (!user?.trim()) {
      return c.json({ ok: false, error: "user query parameter is required" }, 400);
    }

    try {
      const data = await teamsPresence(user);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
    }
  });

  // POST /send -- send a message to a chat
  app.post("/send", async (c) => {
    let body: SendBody = {};
    try {
      body = await c.req.json<SendBody>();
    } catch {
      // Empty or malformed body
    }

    if (!body.chat_id?.trim()) {
      return c.json({ ok: false, error: "chat_id is required in request body" }, 400);
    }
    if (!body.message?.trim()) {
      return c.json({ ok: false, error: "message is required in request body" }, 400);
    }

    try {
      const data = await teamsSend(body.chat_id, body.message);
      return c.json({ ok: true, data });
    } catch (err) {
      return handleToolError(c, err);
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
        `${SERVICE_NAME} listening on port ${info.port}`,
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
