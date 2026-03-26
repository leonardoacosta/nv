import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tools.js";
import { SshError } from "./ssh.js";
import { calendarToday, calendarUpcoming, calendarNext } from "./tools/calendar.js";
import { adoProjects, adoPipelines, adoBuilds } from "./tools/ado.js";

const startedAt = Date.now();

export function createHttpApp(
  registry: ToolRegistry,
  config: ServiceConfig,
): Hono {
  const app = new Hono();

  // Middleware stack
  app.use("*", honoLogger());
  app.use("*", cors({ origin: config.corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    if (err instanceof SshError) {
      return c.json(
        { error: err.message, status: err.httpStatus },
        err.httpStatus,
      );
    }
    return c.json(
      { error: err instanceof Error ? err.message : "Internal Server Error", status: 500 },
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

  // ── Calendar routes ─────────────────────────────────────────────

  app.get("/calendar/today", async (c) => {
    try {
      const result = await calendarToday(config);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/calendar/upcoming", async (c) => {
    try {
      const daysRaw = c.req.query("days");
      const days = daysRaw ? Math.min(14, Math.max(1, parseInt(daysRaw, 10) || 7)) : 7;
      const result = await calendarUpcoming(config, days);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/calendar/next", async (c) => {
    try {
      const result = await calendarNext(config);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  // ── ADO routes ──────────────────────────────────────────────────

  app.get("/ado/projects", async (c) => {
    try {
      const result = await adoProjects(config);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/ado/pipelines", async (c) => {
    try {
      const project = c.req.query("project") || undefined;
      const result = await adoPipelines(config, project);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/ado/builds", async (c) => {
    try {
      const project = c.req.query("project") || undefined;
      const pipeline = c.req.query("pipeline") || undefined;
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 10)) : 10;
      const result = await adoBuilds(config, project, pipeline, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  // Tool dispatch (generic MCP-style endpoint)
  app.post("/tools/:name", async (c) => {
    const name = c.req.param("name");
    try {
      const input = await c.req.json<Record<string, unknown>>().catch(
        () => ({}) as Record<string, unknown>,
      );
      const result = await registry.execute(name, input);
      return c.json({ result, error: null });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ result: null, error: err.message }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ result: null, error: message }, 400);
    }
  });

  return app;
}
