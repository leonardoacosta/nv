import type { Hono } from "hono";

import { getFullRegistry } from "../registry.js";

/**
 * GET /registry
 *
 * Returns the full tool-to-service mapping so callers can discover
 * which tools exist and which service handles each one.
 */
export function registryRoute(app: Hono): void {
  app.get("/registry", (c) => {
    return c.json(getFullRegistry());
  });
}
