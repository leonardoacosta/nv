import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import { getRecentMessages, searchMessages } from "./tools.js";

const startedAt = Date.now();

export function createHttpApp(config: ServiceConfig): Hono {
  const app = new Hono();

  // Middleware stack
  app.use("*", cors({ origin: config.corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    return c.json(
      {
        error: err instanceof Error ? err.message : "Internal Server Error",
        status: 500,
      },
      500,
    );
  });

  // Health endpoint
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: config.serviceName,
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
      version: "0.1.0",
    });
  });

  // GET /recent — get_recent_messages
  app.get("/recent", async (c) => {
    try {
      const channel = c.req.query("channel") || undefined;
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? parseInt(limitRaw, 10) : undefined;

      const results = await getRecentMessages(channel, limit);
      return c.json({ result: results, error: null });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ result: null, error: message }, 500);
    }
  });

  // POST /search — search_messages
  app.post("/search", async (c) => {
    try {
      const body = await c.req.json<{
        query?: string;
        channel?: string;
        limit?: number;
      }>();

      if (!body.query || typeof body.query !== "string" || body.query.trim().length === 0) {
        return c.json({ result: null, error: "query is required and must be a non-empty string" }, 400);
      }

      const results = await searchMessages(body.query, body.channel, body.limit);
      return c.json({ result: results, error: null });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ result: null, error: message }, 500);
    }
  });

  return app;
}
