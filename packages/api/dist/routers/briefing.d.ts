export interface BriefingAction {
    label: string;
    action: string;
    priority?: string;
}
export declare const briefingRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * Get the latest briefing.
     */
    latest: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            entry: null;
        } | {
            entry: {
                id: string;
                generated_at: string;
                content: string;
                suggested_actions: BriefingAction[];
                sources_status: Record<string, string>;
            };
        };
        meta: object;
    }>;
    /**
     * Get briefing history.
     */
    history: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
        };
        output: {
            entries: {
                id: string;
                generated_at: string;
                content: string;
                suggested_actions: BriefingAction[];
                sources_status: Record<string, string>;
            }[];
        };
        meta: object;
    }>;
    /**
     * Trigger briefing generation via the daemon.
     */
    generate: import("@trpc/server").TRPCMutationProcedure<{
        input: void;
        output: {
            success: boolean;
            briefing_id: string;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=briefing.d.ts.map