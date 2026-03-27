export declare const authRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * Verify a token. Returns { ok: true } on success.
     * In dev mode (no DASHBOARD_TOKEN), always succeeds.
     */
    verify: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            token?: string | undefined;
        };
        output: {
            ok: boolean;
            error?: undefined;
        } | {
            ok: boolean;
            error: string;
        };
        meta: object;
    }>;
    /**
     * Logout -- returns { ok: true }.
     * Cookie clearing is handled by the client/catch-all handler.
     */
    logout: import("@trpc/server").TRPCMutationProcedure<{
        input: void;
        output: {
            ok: boolean;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=auth.d.ts.map