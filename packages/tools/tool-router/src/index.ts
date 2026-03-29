import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import pino from "pino";

import { CircuitBreaker } from "./circuit-breaker.js";
import { dispatchRoute } from "./routes/dispatch.js";
import { healthRoute } from "./routes/health.js";
import { metricsRoute } from "./routes/metrics.js";
import { registryRoute } from "./routes/registry.js";
import {
  getAllServices,
  initRegistry,
  refreshRegistry,
  loadRefreshInterval,
} from "./registry.js";

const PORT = parseInt(process.env["PORT"] ?? "4100", 10);
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

// Initialize registry from nv.toml service list before accepting requests
logger.info("Initializing tool registry from configured services...");
try {
  await initRegistry();
} catch (err) {
  logger.error({ err }, "Registry initialization failed — starting with empty registry");
}

// Build per-service circuit breakers from the now-populated registry.
// All services start CLOSED (optimistic — no assumed failures on startup).
const breakers = new Map<string, CircuitBreaker>();
for (const svc of getAllServices()) {
  const breaker = new CircuitBreaker(svc.serviceName);
  breaker.onStateChange = (from, to, reason) => {
    logger.warn({ service: svc.serviceName, from, to, reason }, `Circuit ${to} for ${svc.serviceName}: ${reason}`);
  };
  breakers.set(svc.serviceName, breaker);
}

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
dispatchRoute(app, logger, breakers);
healthRoute(app, logger, breakers);
metricsRoute(app, breakers);
registryRoute(app);

// Start server
const server = serve({ fetch: app.fetch, port: PORT }, (info) => {
  logger.info({ port: info.port, transport: "http" }, `tool-router listening on port ${info.port}`);
});

// Periodic refresh — re-query all services and detect tool changes
const refreshIntervalMs = await loadRefreshInterval();
const refreshTimer = setInterval(async () => {
  try {
    await refreshRegistry();
  } catch (err) {
    logger.error({ err }, "Periodic registry refresh failed");
  }
}, refreshIntervalMs);

// Prevent timer from keeping process alive
refreshTimer.unref();

logger.info(
  { refreshIntervalMs },
  `Registry refresh scheduled every ${refreshIntervalMs / 1000}s`,
);

// Graceful shutdown
const shutdown = () => {
  logger.info("Shutting down...");
  clearInterval(refreshTimer);
  server.close(() => {
    logger.info("Server closed");
    process.exit(0);
  });
};

process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
