/**
 * Dashboard-merged tRPC router definition.
 *
 * Extracted from the catch-all route handler so that the router type
 * can be imported by client modules without pulling in a Next.js
 * route file (which only allows GET/POST/etc. exports).
 */

import { appRouter, mergeRouters } from "@nova/api";
import { ccSessionRouter } from "@/lib/routers/cc-session";
import { resolveRouter } from "@/lib/routers/resolve";

export const dashboardRouter = mergeRouters(
  appRouter,
  ccSessionRouter,
  resolveRouter,
);

export type DashboardRouter = typeof dashboardRouter;
