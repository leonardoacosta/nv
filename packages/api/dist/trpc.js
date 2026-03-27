/**
 * tRPC initialization, context factory, and procedure definitions.
 *
 * - publicProcedure: No auth required (auth.verify, auth.logout)
 * - protectedProcedure: Validates bearer token via timing-safe comparison
 *
 * When DASHBOARD_TOKEN is unset, auth is disabled (dev-mode fallback).
 */
import { initTRPC, TRPCError } from "@trpc/server";
import superjson from "superjson";
import { isAuthEnabled, verifyToken } from "./lib/auth.js";
/**
 * Create the tRPC context from a Request object.
 * Extracts the bearer token from the Authorization header.
 */
export function createTRPCContext(opts) {
    const authHeader = opts.req.headers.get("authorization");
    let token = null;
    if (authHeader?.startsWith("Bearer ")) {
        token = authHeader.slice(7).trim();
    }
    return { token };
}
const t = initTRPC.context().create({
    transformer: superjson,
});
export const createTRPCRouter = t.router;
export const createCallerFactory = t.createCallerFactory;
export const mergeRouters = t.mergeRouters;
/**
 * Public procedure -- no authentication required.
 */
export const publicProcedure = t.procedure;
/**
 * Auth middleware -- validates the bearer token.
 * When DASHBOARD_TOKEN env var is unset, auth is disabled (dev-mode fallback).
 */
const enforceAuth = t.middleware(({ ctx, next }) => {
    // Dev-mode: auth disabled when DASHBOARD_TOKEN is not set
    if (!isAuthEnabled()) {
        return next({ ctx });
    }
    if (!ctx.token) {
        throw new TRPCError({
            code: "UNAUTHORIZED",
            message: "Missing authorization token",
        });
    }
    if (!verifyToken(ctx.token)) {
        throw new TRPCError({
            code: "UNAUTHORIZED",
            message: "Invalid authorization token",
        });
    }
    return next({ ctx });
});
/**
 * Protected procedure -- requires valid bearer token.
 */
export const protectedProcedure = t.procedure.use(enforceAuth);
//# sourceMappingURL=trpc.js.map