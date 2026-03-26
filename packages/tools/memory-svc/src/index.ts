import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import { sql } from "drizzle-orm";
import { db } from "@nova/db";

import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { handleRead } from "./tools/read.js";
import { handleWrite } from "./tools/write.js";
import { handleSearch } from "./tools/search.js";

const config = loadConfig();
const logger = createLogger(config.serviceName, { level: config.logLevel });
const startedAt = Date.now();

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
app.get("/health", async (c) => {
  try {
    await db.execute(sql`SELECT 1`);
    return c.json({
      status: "ok",
      service: config.serviceName,
      uptime: Math.floor((Date.now() - startedAt) / 1000),
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return c.json(
      {
        status: "degraded",
        service: config.serviceName,
        uptime: Math.floor((Date.now() - startedAt) / 1000),
        error: message,
      },
      503,
    );
  }
});

// Tool endpoints
app.post("/read", (c) => handleRead(c, config));
app.post("/write", (c) => handleWrite(c, config, logger));
app.post("/search", (c) => handleSearch(c, config, logger));

// Start server
const server = serve({ fetch: app.fetch, port: config.port }, (info) => {
  logger.info(
    { service: config.serviceName, port: info.port, transport: "http" },
    `${config.serviceName} listening on port ${info.port}`,
  );
});

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
