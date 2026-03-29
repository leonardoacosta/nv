import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import { createLogger } from "./logger.js";
import { probeFleet, summarizeFleet } from "./health.js";
import { runSelfAssessment } from "./self-assess.js";
import { readSoul, writeSoul } from "./soul.js";
import { runTypecheck, runBuild } from "./code-tools.js";

const log = createLogger("meta-svc");
const startedAt = Date.now();

export function createHttpApp(): Hono {
  const app = new Hono();

  // Middleware stack
  app.use("*", honoLogger());
  app.use("*", cors({ origin: "*" }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    log.error({ err }, "Unhandled error");
    return c.json(
      { error: err instanceof Error ? err.message : "Internal Server Error" },
      500,
    );
  });

  // GET /health — standard fleet health endpoint
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: "meta-svc",
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
      version: "0.1.0",
    });
  });

  // Registry endpoint — exposes tool definitions for tool-router self-registration
  app.get("/registry", (c) => {
    return c.json({
      service: "meta-svc",
      tools: [
        {
          name: "check_services",
          description: "Check the health status of all Nova fleet services.",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
          name: "self_assessment_run",
          description: "Run a self-assessment to evaluate Nova's current operational state.",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
          name: "update_soul",
          description: "Update Nova's soul document with new content.",
          inputSchema: {
            type: "object",
            properties: {
              content: { type: "string", description: "New soul document content" },
            },
            required: ["content"],
          },
        },
        {
          name: "typecheck_project",
          description: "Run TypeScript type checking on a project package.",
          inputSchema: {
            type: "object",
            properties: {
              package: { type: "string", description: "Package name to typecheck (omit to check entire workspace)" },
            },
            required: [],
          },
        },
        {
          name: "build_project",
          description: "Build a project package.",
          inputSchema: {
            type: "object",
            properties: {
              package: { type: "string", description: "Package name to build (omit to build entire workspace)" },
            },
            required: [],
          },
        },
      ],
      healthUrl: "http://127.0.0.1:4108/health",
    });
  });

  // GET /services — probe all fleet services
  app.get("/services", async (c) => {
    const services = await probeFleet();
    const summary = summarizeFleet(services);
    return c.json({ services, summary });
  });

  // POST /self-assess — run self-assessment
  app.post("/self-assess", async (c) => {
    const result = await runSelfAssessment();
    return c.json(result);
  });

  // GET /soul — read soul document
  app.get("/soul", async (c) => {
    try {
      const content = await readSoul();
      return c.json({ content });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      log.error({ err }, "Failed to read soul");
      return c.json({ error: message }, 500);
    }
  });

  // POST /soul — update soul document
  app.post("/soul", async (c) => {
    let body: { content?: string };
    try {
      body = (await c.req.json()) as { content?: string };
    } catch {
      return c.json({ error: "Invalid JSON body" }, 400);
    }

    if (!body.content || typeof body.content !== "string" || body.content.trim().length === 0) {
      return c.json({ error: "content is required and must be a non-empty string" }, 400);
    }

    try {
      await writeSoul(body.content);
      return c.json({
        ok: true,
        bytes: Buffer.byteLength(body.content, "utf-8"),
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      log.error({ err }, "Failed to write soul");
      return c.json({ error: message }, 500);
    }
  });

  // POST /typecheck — run pnpm typecheck
  app.post("/typecheck", async (c) => {
    let body: { package?: string } = {};
    try {
      body = (await c.req.json()) as { package?: string };
    } catch {
      // empty body is fine — runs on whole workspace
    }
    const result = await runTypecheck(body.package);
    return c.json(result, result.success ? 200 : 422);
  });

  // POST /build — run pnpm build
  app.post("/build", async (c) => {
    let body: { package?: string } = {};
    try {
      body = (await c.req.json()) as { package?: string };
    } catch {
      // empty body is fine — runs on whole workspace
    }
    const result = await runBuild(body.package);
    return c.json(result, result.success ? 200 : 422);
  });

  return app;
}

export async function startServer(port: number): Promise<void> {
  const app = createHttpApp();

  const server = serve({ fetch: app.fetch, port }, (info) => {
    log.info(
      { service: "meta-svc", port: info.port, transport: "http" },
      `meta-svc listening on :${info.port}`,
    );
  });

  // Graceful shutdown
  const shutdown = () => {
    log.info("Shutting down...");
    server.close(() => {
      log.info("Server closed");
      process.exit(0);
    });
  };

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);
}
