/**
 * Catch-all tRPC API route handler.
 *
 * Router definition lives in @/lib/trpc/router to avoid exporting
 * non-route symbols from a Next.js route file.
 */

import { fetchRequestHandler } from "@trpc/server/adapters/fetch";
import { createTRPCContext } from "@nova/api";
import { dashboardRouter } from "@/lib/trpc/router";

function handler(req: Request) {
  return fetchRequestHandler({
    endpoint: "/api/trpc",
    req,
    router: dashboardRouter,
    createContext: () => createTRPCContext({ req }),
  });
}

export { handler as GET, handler as POST };
