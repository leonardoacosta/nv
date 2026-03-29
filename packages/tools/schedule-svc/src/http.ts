import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tool-registry.js";

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
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
      version: "0.1.0",
    });
  });

  // Registry endpoint — exposes tool definitions for tool-router self-registration
  app.get("/registry", (c) => {
    return c.json({
      service: config.serviceName,
      tools: registry.list().map((t) => ({
        name: t.name,
        description: t.description,
        inputSchema: t.inputSchema,
      })),
      healthUrl: `http://127.0.0.1:${config.servicePort}/health`,
    });
  });

  // --- Reminder routes ---

  app.post("/reminders", async (c) => {
    const body = await c.req.json<Record<string, unknown>>();
    const result = await registry.execute("set_reminder", body);
    return c.json({ result });
  });

  app.delete("/reminders/:id", async (c) => {
    const id = c.req.param("id");
    const result = await registry.execute("cancel_reminder", { id });
    return c.json({ result });
  });

  app.get("/reminders", async (c) => {
    const status = c.req.query("status") ?? "active";
    const result = await registry.execute("list_reminders", { status });
    return c.json({ result: JSON.parse(result) });
  });

  // --- Schedule routes ---

  app.post("/schedules", async (c) => {
    const body = await c.req.json<Record<string, unknown>>();
    const result = await registry.execute("add_schedule", body);
    return c.json({ result });
  });

  app.patch("/schedules/:id", async (c) => {
    const id = c.req.param("id");
    const body = await c.req.json<Record<string, unknown>>();
    const result = await registry.execute("modify_schedule", {
      id,
      updates: body,
    });
    return c.json({ result });
  });

  app.delete("/schedules/:id", async (c) => {
    const id = c.req.param("id");
    const result = await registry.execute("remove_schedule", { id });
    return c.json({ result });
  });

  app.get("/schedules", async (c) => {
    const activeParam = c.req.query("active");
    const active = activeParam === undefined ? true : activeParam === "true";
    const result = await registry.execute("list_schedules", { active });
    return c.json({ result: JSON.parse(result) });
  });

  // --- Session routes ---

  app.post("/sessions/start", async (c) => {
    const body = await c.req.json<Record<string, unknown>>();
    const result = await registry.execute("start_session", body);
    return c.json({ result });
  });

  app.post("/sessions/stop", async (c) => {
    const body = await c.req.json<Record<string, unknown>>().catch(() => ({}));
    const result = await registry.execute("stop_session", body);
    return c.json({ result });
  });

  return app;
}
