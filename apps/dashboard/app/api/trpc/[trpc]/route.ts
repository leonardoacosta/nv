/**
 * Catch-all tRPC API route handler.
 *
 * Merges the @nova/api appRouter with dashboard-local routers
 * (cc-session, resolve) that depend on Docker/entity-resolution.
 */

import { fetchRequestHandler } from "@trpc/server/adapters/fetch";
import { appRouter, createTRPCContext } from "@nova/api";
import { mergeRouters } from "@nova/api";
import { ccSessionRouter } from "@/lib/routers/cc-session";
import { resolveRouter } from "@/lib/routers/resolve";

export const dashboardRouter = mergeRouters(
  appRouter,
  ccSessionRouter,
  resolveRouter,
);

export type DashboardRouter = typeof dashboardRouter;

function handler(req: Request) {
  return fetchRequestHandler({
    endpoint: "/api/trpc",
    req,
    router: dashboardRouter,
    createContext: () => createTRPCContext({ req }),
  });
}

export { handler as GET, handler as POST };
