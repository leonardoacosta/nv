/**
 * @nova/api -- tRPC API layer for the Nova dashboard.
 *
 * Barrel exports for consumption by apps/dashboard and other consumers.
 */
export { appRouter, createCaller } from "./root.js";
export type { AppRouter, RouterOutputs, RouterInputs } from "./root.js";
export { createTRPCContext, createTRPCRouter, publicProcedure, protectedProcedure, mergeRouters, } from "./trpc.js";
export type { TRPCContext } from "./trpc.js";
//# sourceMappingURL=index.d.ts.map