import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";

import type { AdapterRegistry } from "./adapters/registry.js";
import type { ChannelName, SendRequest } from "./types.js";
import type { Logger } from "./logger.js";

const startedAt = Date.now();

export function createHttpApp(
  registry: AdapterRegistry,
  config: { serviceName: string; servicePort: number; corsOrigin: string },
  logger: Logger,
): Hono {
  const app = new Hono();

  // Middleware
  app.use("*", cors({ origin: config.corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    logger.error({ err }, "Unhandled error");
    return c.json(
      { error: err instanceof Error ? err.message : "Internal Server Error" },
      500,
    );
  });

  // Health endpoint
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: config.serviceName,
      port: config.servicePort,
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
    });
  });

  // Registry endpoint — exposes tool definitions for tool-router self-registration
  app.get("/registry", (c) => {
    return c.json({
      service: "channels-svc",
      tools: [
        {
          name: "list_channels",
          description: "List available messaging channels (Telegram, Discord, Teams, etc.) with connection status.",
          inputSchema: {
            type: "object",
            properties: {},
            required: [],
          },
        },
        {
          name: "send_to_channel",
          description: "Send a message to a specific channel. Requires operator confirmation for outbound messages.",
          inputSchema: {
            type: "object",
            properties: {
              channel: { type: "string", description: "Channel name" },
              target: { type: "string", description: "Target identifier (chat ID, channel ID, email address)" },
              message: { type: "string", description: "Message body" },
            },
            required: ["channel", "target", "message"],
          },
        },
      ],
      healthUrl: `http://127.0.0.1:${config.servicePort}/health`,
    });
  });

  // List channels
  app.get("/channels", (c) => {
    return c.json({ channels: registry.list() });
  });

  // Send message
  app.post("/send", async (c) => {
    const body = await c.req.json<SendRequest>();

    const { channel, target, message } = body;

    if (!channel || !target || !message) {
      return c.json(
        { ok: false, error: "Missing required fields: channel, target, message" },
        400,
      );
    }

    const channelName = channel as ChannelName;
    const adapter = registry.get(channelName);

    if (!adapter) {
      return c.json(
        { ok: false, error: `Channel not found: ${channel}` },
        404,
      );
    }

    // Check direction supports outbound
    if (adapter.direction === "inbound") {
      return c.json(
        { ok: false, error: `Channel ${channel} does not support outbound messages` },
        400,
      );
    }

    // Check status
    const status = adapter.status();
    if (status !== "connected") {
      return c.json(
        { ok: false, error: `Channel ${channel} is ${status}` },
        503,
      );
    }

    try {
      await adapter.send(target, message);
      logger.info(
        { channel, target: target.slice(0, 20) },
        "Message sent",
      );
      return c.json({ ok: true, channel, target });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "Unknown error";
      logger.error(
        { channel, target: target.slice(0, 20), error: errorMessage },
        "Failed to send message",
      );
      return c.json({ ok: false, error: errorMessage }, 502);
    }
  });

  return app;
}
