import { Hono } from "hono";
import type { Context } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tools.js";
import { SshError } from "./ssh.js";
import { parseIntOr, clamp } from "./utils.js";

const startedAt = Date.now();

/**
 * Wraps a route handler so SSH/unknown errors are caught and returned as JSON.
 * Eliminates the try/catch boilerplate that was duplicated 20+ times.
 */
function safe(fn: (c: Context) => Promise<Response>) {
  return async (c: Context) => {
    try {
      return await fn(c);
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  };
}

export function createHttpApp(
  registry: ToolRegistry,
  config: ServiceConfig,
): Hono {
  const app = new Hono();

  // Middleware stack
  app.use("*", honoLogger());
  app.use("*", cors({ origin: config.corsOrigin }));
  app.use("*", secureHeaders());

  // Global error handler (catches anything not handled by safe())
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
      tools: registry.list().length,
      version: "0.2.0",
    });
  });

  // ── Generic tool dispatch ─────────────────────────────────────
  // All tools are accessible via POST /tools/:name.
  // The named routes below are convenience aliases.

  app.post("/tools/:name", safe(async (c) => {
    const name = c.req.param("name");
    const input = await c.req.json<Record<string, unknown>>().catch(
      () => ({}) as Record<string, unknown>,
    );
    const result = await registry.execute(name, input);
    return c.json({ result, error: null });
  }));

  // ── Convenience REST routes ───────────────────────────────────
  // These delegate to the same tool handlers via the registry.
  // Kept for backward compat with direct HTTP callers (dashboard, Telegram).

  // Calendar
  app.get("/calendar/today", safe(async (c) => {
    const result = await registry.execute("calendar_today", {});
    return c.json({ result });
  }));

  app.get("/calendar/upcoming", safe(async (c) => {
    const days = clamp(parseIntOr(c.req.query("days"), 7), 1, 14);
    const result = await registry.execute("calendar_upcoming", { days });
    return c.json({ result });
  }));

  app.get("/calendar/next", safe(async (c) => {
    const result = await registry.execute("calendar_next", {});
    return c.json({ result });
  }));

  // ADO
  app.get("/ado/projects", safe(async (c) => {
    const result = await registry.execute("ado_projects", {});
    return c.json({ result });
  }));

  app.get("/ado/pipelines", safe(async (c) => {
    const result = await registry.execute("ado_pipelines", {
      project: c.req.query("project") || undefined,
    });
    return c.json({ result });
  }));

  app.get("/ado/builds", safe(async (c) => {
    const result = await registry.execute("ado_builds", {
      project: c.req.query("project") || undefined,
      pipeline: c.req.query("pipeline") || undefined,
      limit: clamp(parseIntOr(c.req.query("limit"), 10), 1, 50),
    });
    return c.json({ result });
  }));

  app.get("/ado/work-items", safe(async (c) => {
    const result = await registry.execute("ado_work_items", {
      project: c.req.query("project") || undefined,
      state: c.req.query("state") || undefined,
      type: c.req.query("type") || undefined,
      limit: clamp(parseIntOr(c.req.query("limit"), 20), 1, 50),
    });
    return c.json({ result });
  }));

  app.get("/ado/repos", safe(async (c) => {
    const result = await registry.execute("ado_repos", {
      project: c.req.query("project") || undefined,
    });
    return c.json({ result });
  }));

  app.get("/ado/pull-requests", safe(async (c) => {
    const result = await registry.execute("ado_pull_requests", {
      project: c.req.query("project") || undefined,
      status: c.req.query("status") || undefined,
    });
    return c.json({ result });
  }));

  app.get("/ado/build-logs/:buildId", safe(async (c) => {
    const buildId = parseInt(c.req.param("buildId"), 10);
    if (isNaN(buildId)) {
      return c.json({ error: "buildId must be a number", status: 400 }, 400);
    }
    const result = await registry.execute("ado_build_logs", {
      build_id: buildId,
      project: c.req.query("project") || undefined,
    });
    return c.json({ result });
  }));

  // PIM
  app.get("/pim/status", safe(async (c) => {
    const result = await registry.execute("pim_status", {});
    return c.json({ result });
  }));

  app.post("/pim/activate", safe(async (c) => {
    const body = await c.req.json<{ role_number?: number; justification?: string }>();
    if (typeof body.role_number !== "number") {
      return c.json({ error: "role_number is required", status: 400 }, 400);
    }
    const result = await registry.execute("pim_activate", {
      role_number: body.role_number,
      justification: body.justification || undefined,
    });
    return c.json({ result });
  }));

  app.post("/pim/activate-all", safe(async (c) => {
    const body = await c.req.json<{ justification?: string }>().catch(() => ({}));
    const result = await registry.execute("pim_activate_all", {
      justification: (body as { justification?: string }).justification || undefined,
    });
    return c.json({ result });
  }));

  // Mail
  app.get("/mail/inbox", safe(async (c) => {
    const result = await registry.execute("outlook_inbox", {
      limit: clamp(parseIntOr(c.req.query("limit"), 10), 1, 50),
    });
    return c.json({ result });
  }));

  app.get("/mail/read/:messageId", safe(async (c) => {
    const result = await registry.execute("outlook_read", {
      message_id: c.req.param("messageId"),
    });
    return c.json({ result });
  }));

  app.post("/mail/search", safe(async (c) => {
    const body = await c.req.json<{ query?: string; limit?: number }>();
    if (!body.query) {
      return c.json({ error: "query is required", status: 400 }, 400);
    }
    const result = await registry.execute("outlook_search", {
      query: body.query,
      limit: body.limit ? clamp(body.limit, 1, 50) : 10,
    });
    return c.json({ result });
  }));

  app.get("/mail/folders", safe(async (c) => {
    const result = await registry.execute("outlook_folders", {});
    return c.json({ result });
  }));

  app.get("/mail/sent", safe(async (c) => {
    const result = await registry.execute("outlook_sent", {
      limit: clamp(parseIntOr(c.req.query("limit"), 10), 1, 50),
    });
    return c.json({ result });
  }));

  app.get("/mail/folder/:folderId", safe(async (c) => {
    const result = await registry.execute("outlook_folder", {
      folder_id: c.req.param("folderId"),
      limit: clamp(parseIntOr(c.req.query("limit"), 10), 1, 50),
    });
    return c.json({ result });
  }));

  app.post("/mail/flag", safe(async (c) => {
    const body = await c.req.json<{ message_id?: string }>();
    if (!body.message_id) {
      return c.json({ error: "message_id is required", status: 400 }, 400);
    }
    const result = await registry.execute("outlook_flag", {
      message_id: body.message_id,
    });
    return c.json({ result });
  }));

  app.post("/mail/move", safe(async (c) => {
    const body = await c.req.json<{ message_id?: string; destination_folder?: string }>();
    if (!body.message_id) {
      return c.json({ error: "message_id is required", status: 400 }, 400);
    }
    if (!body.destination_folder) {
      return c.json({ error: "destination_folder is required", status: 400 }, 400);
    }
    const result = await registry.execute("outlook_move", {
      message_id: body.message_id,
      destination_folder: body.destination_folder,
    });
    return c.json({ result });
  }));

  app.get("/mail/unread", safe(async (c) => {
    const result = await registry.execute("outlook_unread", {
      limit: clamp(parseIntOr(c.req.query("limit"), 10), 1, 50),
    });
    return c.json({ result });
  }));

  return app;
}
