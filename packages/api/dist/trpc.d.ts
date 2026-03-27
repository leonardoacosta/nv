/**
 * tRPC initialization, context factory, and procedure definitions.
 *
 * - publicProcedure: No auth required (auth.verify, auth.logout)
 * - protectedProcedure: Validates bearer token via timing-safe comparison
 *
 * When DASHBOARD_TOKEN is unset, auth is disabled (dev-mode fallback).
 */
export interface TRPCContext {
    /**
     * Bearer token extracted from Authorization header or null.
     * Available after context creation; auth validation happens in middleware.
     */
    token: string | null;
}
/**
 * Create the tRPC context from a Request object.
 * Extracts the bearer token from the Authorization header.
 */
export declare function createTRPCContext(opts: {
    req: Request;
}): TRPCContext;
export declare const createTRPCRouter: import("@trpc/server").TRPCRouterBuilder<{
    ctx: TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}>;
export declare const createCallerFactory: import("@trpc/server").TRPCRouterCallerFactory<{
    ctx: TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}>;
export declare const mergeRouters: <TRouters extends import("@trpc/server").AnyRouter[]>(...routerList: TRouters) => import("@trpc/server").TRPCMergeRouters<TRouters>;
/**
 * Public procedure -- no authentication required.
 */
export declare const publicProcedure: import("@trpc/server").TRPCProcedureBuilder<TRPCContext, object, object, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, false>;
/**
 * Protected procedure -- requires valid bearer token.
 */
export declare const protectedProcedure: import("@trpc/server").TRPCProcedureBuilder<TRPCContext, object, {
    token: string | null;
}, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, import("@trpc/server").TRPCUnsetMarker, false>;
//# sourceMappingURL=trpc.d.ts.map