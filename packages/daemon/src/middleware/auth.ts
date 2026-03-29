import type { MiddlewareHandler } from "hono";

/**
 * Bearer token authentication middleware factory.
 *
 * Checks the `Authorization: Bearer <token>` header on every request.
 * Returns HTTP 401 `{ error: "Unauthorized" }` if the header is absent or
 * does not match the provided token exactly.
 *
 * Note: health and readiness endpoints (`GET /health`, `GET /ready`) are
 * excluded from auth checks and remain public.
 */
export function bearerAuth(token: string): MiddlewareHandler {
  return async function bearerAuthMiddleware(c, next) {
    // Exclude public health/ready endpoints
    const path = new URL(c.req.url).pathname;
    if (c.req.method === "GET" && (path === "/health" || path === "/ready")) {
      await next();
      return;
    }

    const authHeader = c.req.header("Authorization");
    if (!authHeader || authHeader !== `Bearer ${token}`) {
      return c.json({ error: "Unauthorized" }, 401);
    }

    await next();
  };
}
