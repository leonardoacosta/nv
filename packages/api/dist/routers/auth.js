import { z } from "zod";
import { createTRPCRouter, publicProcedure } from "../trpc.js";
import { isAuthEnabled, verifyToken } from "../lib/auth.js";
export const authRouter = createTRPCRouter({
    /**
     * Verify a token. Returns { ok: true } on success.
     * In dev mode (no DASHBOARD_TOKEN), always succeeds.
     */
    verify: publicProcedure
        .input(z.object({ token: z.string().optional() }))
        .mutation(({ input }) => {
        // Dev mode bypass
        if (!isAuthEnabled()) {
            return { ok: true };
        }
        if (!input.token) {
            return { ok: false, error: "Token required" };
        }
        if (!verifyToken(input.token)) {
            return { ok: false, error: "Invalid token" };
        }
        return { ok: true };
    }),
    /**
     * Logout -- returns { ok: true }.
     * Cookie clearing is handled by the client/catch-all handler.
     */
    logout: publicProcedure.mutation(() => {
        return { ok: true };
    }),
});
//# sourceMappingURL=auth.js.map