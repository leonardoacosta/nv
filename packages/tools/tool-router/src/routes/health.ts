import type { Hono } from "hono";
import type { Logger } from "pino";

import type { CircuitBreaker, CircuitState } from "../circuit-breaker.js";
import { getAllServices } from "../registry.js";

const HEALTH_TIMEOUT_MS = 3000;

interface ServiceHealth {
  status: "healthy" | "unreachable";
  url: string;
  latency_ms: number | null;
  circuitBreakerState: CircuitState;
}

/**
 * GET /health
 *
 * Calls GET {serviceUrl}/health on each registered service in parallel,
 * with a 3-second per-service timeout.
 *
 * After each check, updates circuit breaker state:
 *   - 200 response → breaker.onSuccess()
 *   - unreachable / non-200 → breaker.onFailure()
 *
 * Returns aggregate status:
 *   "healthy"   — all services responded 200
 *   "degraded"  — at least one unreachable or non-200
 *   "unhealthy" — zero services responded
 */
export function healthRoute(
  app: Hono,
  logger: Logger,
  breakers: Map<string, CircuitBreaker>,
): void {
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
        const breaker = breakers.get(serviceName);
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
            breaker?.onSuccess();
            results[serviceName] = {
              status: "healthy",
              url: serviceUrl,
              latency_ms: latency,
              circuitBreakerState: breaker?.state ?? "CLOSED",
            };
            healthyCount++;
          } else {
            breaker?.onFailure();
            results[serviceName] = {
              status: "unreachable",
              url: serviceUrl,
              latency_ms: latency,
              circuitBreakerState: breaker?.state ?? "CLOSED",
            };
          }
        } catch {
          breaker?.onFailure();
          results[serviceName] = {
            status: "unreachable",
            url: serviceUrl,
            latency_ms: null,
            circuitBreakerState: breaker?.state ?? "CLOSED",
          };
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
