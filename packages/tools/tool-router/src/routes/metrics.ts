import type { Hono } from "hono";

import type { CircuitBreaker, CircuitState } from "../circuit-breaker.js";

interface ServiceMetrics {
  totalRequests: number;
  totalFailures: number;
  circuitTrips: number;
  lastTripAt: string | null; // ISO 8601
  circuitState: CircuitState;
}

/**
 * In-memory per-service metrics counters.
 * Keyed by serviceName, updated by metricsRecorder helpers below.
 */
const _metrics = new Map<string, ServiceMetrics>();

/** Record a request (success or failure) for a service. */
export function recordRequest(serviceName: string, failed: boolean): void {
  const m = _metrics.get(serviceName);
  if (!m) return;
  m.totalRequests++;
  if (failed) {
    m.totalFailures++;
  }
}

/** Record a circuit trip (CLOSED → OPEN) for a service. */
export function recordCircuitTrip(serviceName: string): void {
  const m = _metrics.get(serviceName);
  if (!m) return;
  m.circuitTrips++;
  m.lastTripAt = new Date().toISOString();
}

/**
 * Initialize metrics tracking for all services and wire circuit trip callbacks.
 * Must be called once in index.ts after breakers are created.
 */
export function initMetrics(breakers: Map<string, CircuitBreaker>): void {
  for (const [serviceName, breaker] of breakers) {
    _metrics.set(serviceName, {
      totalRequests: 0,
      totalFailures: 0,
      circuitTrips: 0,
      lastTripAt: null,
      circuitState: breaker.state,
    });

    // Hook into state transitions to count trips and keep circuitState current
    const existingCallback = breaker.onStateChange;
    breaker.onStateChange = (from, to, reason) => {
      existingCallback?.(from, to, reason);
      const m = _metrics.get(serviceName);
      if (m) {
        m.circuitState = to;
        if (from === "CLOSED" && to === "OPEN") {
          m.circuitTrips++;
          m.lastTripAt = new Date().toISOString();
        }
      }
    };
  }
}

const _startTime = Date.now();

/**
 * GET /metrics
 *
 * Exposes per-service counters and process uptime (seconds).
 */
export function metricsRoute(app: Hono, breakers: Map<string, CircuitBreaker>): void {
  // Initialize metrics tracking when route is wired
  initMetrics(breakers);

  app.get("/metrics", (c) => {
    const services: Record<string, ServiceMetrics> = {};

    for (const [serviceName, breaker] of breakers) {
      const m = _metrics.get(serviceName);
      if (m) {
        // Keep circuitState live-synced from breaker on every read
        m.circuitState = breaker.state;
        services[serviceName] = { ...m };
      }
    }

    return c.json({
      uptime_secs: Math.floor((Date.now() - _startTime) / 1000),
      services,
    });
  });
}
