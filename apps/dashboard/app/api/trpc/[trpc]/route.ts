/**
 * Catch-all tRPC API route handler.
 *
 * Merges the @nova/api appRouter with dashboard-local routers
 * (cc-session, resolve) that depend on Docker/entity-resolution.
 */

import { fetchRequestHandler } from "@trpc/server/adapters/fetch";
import { appRouter, createTRPCContext } from "@nova/api";

function handler(req: Request) {
  return fetchRequestHandler({
    endpoint: "/api/trpc",
    req,
    router: appRouter,
    createContext: () => createTRPCContext({ req }),
  });
}

export { handler as GET, handler as POST };
