import type { MiddlewareHandler } from "hono";

// Shared in-memory store: routeKey → array of recent request timestamps (ms)
const store = new Map<string, number[]>();

/**
 * Sliding-window rate limiter middleware factory.
 *
 * Tracks requests per `routeKey` in an in-memory Map. On each request:
 * 1. Prunes timestamps older than 60 seconds from the bucket.
 * 2. If the bucket has >= `limit` timestamps, returns HTTP 429.
 * 3. Otherwise, records the current timestamp and calls `next()`.
 *
 * Each endpoint uses its own independent bucket (keyed by `routeKey`).
 * The store resets on daemon restart — acceptable for a single-process daemon.
 *
 * @param routeKey - Unique string key identifying the rate-limited route (e.g. "/chat")
 * @param limit    - Maximum number of requests allowed per 60-second window
 */
export function rateLimiter(routeKey: string, limit: number): MiddlewareHandler {
  const windowMs = 60_000; // 60-second sliding window

  return async function rateLimiterMiddleware(c, next) {
    const now = Date.now();

    // Get or create the bucket for this route
    let timestamps = store.get(routeKey) ?? [];

    // Prune entries outside the window
    timestamps = timestamps.filter((ts) => now - ts < windowMs);

    if (timestamps.length >= limit) {
      const oldestTs = timestamps[0];
      // retryAfter: seconds until the oldest entry expires
      const retryAfterSecs = Math.ceil((oldestTs + windowMs - now) / 1000);
      store.set(routeKey, timestamps);
      return c.json(
        {
          error: `Rate limit exceeded. Max ${limit} requests per minute.`,
          retryAfter: retryAfterSecs,
        },
        429,
      );
    }

    timestamps.push(now);
    store.set(routeKey, timestamps);

    await next();
  };
}
