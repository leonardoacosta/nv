/**
 * @nova/api -- tRPC API layer for the Nova dashboard.
 *
 * Barrel exports for consumption by apps/dashboard and other consumers.
 */

// Router
export { appRouter, createCaller } from "./root.js";
export type { AppRouter, RouterOutputs, RouterInputs } from "./root.js";

// Context and procedures (for catch-all handler and dashboard-local routers)
export {
  createTRPCContext,
  createTRPCRouter,
  publicProcedure,
  protectedProcedure,
  mergeRouters,
} from "./trpc.js";
export type { TRPCContext } from "./trpc.js";
