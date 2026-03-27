import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tools.js";
import { SshError } from "./ssh.js";
import { calendarToday, calendarUpcoming, calendarNext } from "./tools/calendar.js";
import { adoProjects, adoPipelines, adoBuilds } from "./tools/ado.js";
import { adoWorkItems, adoRepos, adoPullRequests, adoBuildLogs } from "./tools/ado-extended.js";
import { outlookInbox, outlookRead, outlookSearch, outlookFolders, outlookSent, outlookFolder, outlookFlag, outlookMove, outlookUnread } from "./tools/mail.js";
import { pimStatus, pimActivate, pimActivateAll } from "./tools/pim.js";

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

  // ── PIM routes ─────────────────────────────────────────────────

  app.get("/pim/status", async (c) => {
    try {
      const result = await pimStatus(config);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.post("/pim/activate", async (c) => {
    try {
      const body = await c.req.json<{ role_number?: number; justification?: string }>();
      const roleNumber = body.role_number;
      if (typeof roleNumber !== "number") {
        return c.json({ error: "role_number is required", status: 400 }, 400);
      }
      const justification = body.justification || undefined;
      const result = await pimActivate(config, roleNumber, justification);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.post("/pim/activate-all", async (c) => {
    try {
      const body = await c.req.json<{ justification?: string }>().catch(() => ({}));
      const justification = (body as { justification?: string }).justification || undefined;
      const result = await pimActivateAll(config, justification);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  // ── ADO Extended routes ───────────────────────────────────────

  app.get("/ado/work-items", async (c) => {
    try {
      const project = c.req.query("project") || undefined;
      const state = c.req.query("state") || undefined;
      const type = c.req.query("type") || undefined;
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 20)) : 20;
      const result = await adoWorkItems(config, project, state, type, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/ado/repos", async (c) => {
    try {
      const project = c.req.query("project") || undefined;
      const result = await adoRepos(config, project);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/ado/pull-requests", async (c) => {
    try {
      const project = c.req.query("project") || undefined;
      const status = c.req.query("status") || undefined;
      const result = await adoPullRequests(config, project, status);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/ado/build-logs/:buildId", async (c) => {
    try {
      const buildIdRaw = c.req.param("buildId");
      const buildId = parseInt(buildIdRaw, 10);
      if (isNaN(buildId)) {
        return c.json({ error: "buildId must be a number", status: 400 }, 400);
      }
      const project = c.req.query("project") || undefined;
      const result = await adoBuildLogs(config, buildId, project);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  // ── Mail routes ────────────────────────────────────────────────

  app.get("/mail/inbox", async (c) => {
    try {
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 10)) : 10;
      const result = await outlookInbox(config, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/mail/read/:messageId", async (c) => {
    try {
      const messageId = c.req.param("messageId");
      const result = await outlookRead(config, messageId);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.post("/mail/search", async (c) => {
    try {
      const body = await c.req.json<{ query?: string; limit?: number }>();
      const query = body.query;
      if (!query) {
        return c.json({ error: "query is required", status: 400 }, 400);
      }
      const limit = body.limit ? Math.min(50, Math.max(1, body.limit)) : 10;
      const result = await outlookSearch(config, query, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/mail/folders", async (c) => {
    try {
      const result = await outlookFolders(config);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/mail/sent", async (c) => {
    try {
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 10)) : 10;
      const result = await outlookSent(config, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/mail/folder/:folderId", async (c) => {
    try {
      const folderId = c.req.param("folderId");
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 10)) : 10;
      const result = await outlookFolder(config, folderId, limit);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.post("/mail/flag", async (c) => {
    try {
      const body = await c.req.json<{ message_id?: string }>();
      const messageId = body.message_id;
      if (!messageId) {
        return c.json({ error: "message_id is required", status: 400 }, 400);
      }
      const result = await outlookFlag(config, messageId);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.post("/mail/move", async (c) => {
    try {
      const body = await c.req.json<{ message_id?: string; destination_folder?: string }>();
      const messageId = body.message_id;
      const destinationFolder = body.destination_folder;
      if (!messageId) {
        return c.json({ error: "message_id is required", status: 400 }, 400);
      }
      if (!destinationFolder) {
        return c.json({ error: "destination_folder is required", status: 400 }, 400);
      }
      const result = await outlookMove(config, messageId, destinationFolder);
      return c.json({ result });
    } catch (err) {
      if (err instanceof SshError) {
        return c.json({ error: err.message, status: err.httpStatus }, err.httpStatus);
      }
      const message = err instanceof Error ? err.message : "Unknown error";
      return c.json({ error: message, status: 500 }, 500);
    }
  });

  app.get("/mail/unread", async (c) => {
    try {
      const limitRaw = c.req.query("limit");
      const limit = limitRaw ? Math.min(50, Math.max(1, parseInt(limitRaw, 10) || 10)) : 10;
      const result = await outlookUnread(config, limit);
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
