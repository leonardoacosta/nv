import type { Hono } from "hono";
import type { Logger } from "pino";

import { getAllServices } from "../registry.js";

const HEALTH_TIMEOUT_MS = 3000;

interface ServiceHealth {
  status: "healthy" | "unreachable";
  url: string;
  latency_ms: number | null;
}

/**
 * GET /health
 *
 * Calls GET {serviceUrl}/health on each registered service in parallel,
 * with a 3-second per-service timeout.
 *
 * Returns aggregate status:
 *   "healthy"   — all services responded 200
 *   "degraded"  — at least one unreachable or non-200
 *   "unhealthy" — zero services responded
 */
export function healthRoute(app: Hono, logger: Logger): void {
  app.get("/health", async (c) => {
    const services = getAllServices();
    const uniqueServices = new Map<string, string>();
    for (const svc of services) {
      uniqueServices.set(svc.serviceName, svc.serviceUrl);
    }

    const results: Record<string, ServiceHealth> = {};
    let healthyCount = 0;
    const totalCount = uniqueServices.size;

    const checks = Array.from(uniqueServices.entries()).map(
      async ([serviceName, serviceUrl]) => {
        const start = Date.now();
        try {
          const controller = new AbortController();
          const timeout = setTimeout(() => controller.abort(), HEALTH_TIMEOUT_MS);

          const resp = await fetch(`${serviceUrl}/health`, {
            signal: controller.signal,
          });
          clearTimeout(timeout);

          const latency = Date.now() - start;

          if (resp.ok) {
            results[serviceName] = { status: "healthy", url: serviceUrl, latency_ms: latency };
            healthyCount++;
          } else {
            results[serviceName] = { status: "unreachable", url: serviceUrl, latency_ms: latency };
          }
        } catch {
          results[serviceName] = { status: "unreachable", url: serviceUrl, latency_ms: null };
        }
      },
    );

    await Promise.all(checks);

    let status: "healthy" | "degraded" | "unhealthy";
    if (healthyCount === totalCount) {
      status = "healthy";
    } else if (healthyCount === 0) {
      status = "unhealthy";
    } else {
      status = "degraded";
    }

    logger.info({ status, healthyCount, totalCount }, "Health check completed");

    return c.json({
      status,
      services: results,
      healthy_count: healthyCount,
      total_count: totalCount,
    });
  });
}
