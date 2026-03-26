import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger as honoLogger } from "hono/logger";
import { secureHeaders } from "hono/secure-headers";

import { createLogger } from "./logger.js";
import { probeFleet, summarizeFleet } from "./health.js";
import { runSelfAssessment } from "./self-assess.js";
import { readSoul, writeSoul } from "./soul.js";

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
