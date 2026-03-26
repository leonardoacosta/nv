import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import pino from "pino";

import { dispatchRoute } from "./routes/dispatch.js";
import { healthRoute } from "./routes/health.js";
import { registryRoute } from "./routes/registry.js";

const PORT = parseInt(process.env["PORT"] ?? "4000", 10);
const isDev = process.env["NODE_ENV"] !== "production";

const logger = pino({
  name: "tool-router",
  level: process.env["LOG_LEVEL"] ?? "info",
  ...(isDev
    ? {
        transport: {
          target: "pino-pretty",
          options: { colorize: true, translateTime: "SYS:standard", ignore: "pid,hostname" },
        },
      }
    : {}),
});

const app = new Hono();

// Middleware
app.use("*", cors({ origin: process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev" }));
app.use("*", secureHeaders());

// Global error handler
app.onError((err, c) => {
  logger.error({ err }, "Unhandled error");
  return c.json(
    { error: err instanceof Error ? err.message : "Internal Server Error" },
    500,
  );
});

// Routes
dispatchRoute(app, logger);
healthRoute(app, logger);
registryRoute(app);

// Start server
const server = serve({ fetch: app.fetch, port: PORT }, (info) => {
  logger.info({ port: info.port, transport: "http" }, `tool-router listening on port ${info.port}`);
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
