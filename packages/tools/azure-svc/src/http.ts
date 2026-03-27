import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tools.js";
import { SshError } from "./ssh.js";
import { runAzureCli } from "./tools/azure-cli.js";

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

  // ── Azure CLI route ─────────────────────────────────────────────
  app.post("/az", async (c) => {
    try {
      const body = await c.req.json<{ command?: string }>();
      const command = body?.command;

      if (typeof command !== "string" || !command.trim()) {
        return c.json(
          { result: null, error: "Missing required 'command' field. Example: az vm list" },
          400,
        );
      }

      const result = await runAzureCli(config, command);
      return c.json({ result, error: null });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ result: null, error: err.message }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ result: null, error: message }, 400);
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
