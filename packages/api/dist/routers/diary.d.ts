export declare const diaryRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List diary entries with optional date/limit filters.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
            date?: string | undefined;
        };
        output: {
            date: string;
            entries: {
                time: string;
                trigger_type: string;
                trigger_source: string;
                channel_source: string;
                slug: string;
                tools_called: string[];
                result_summary: string;
                response_latency_ms: number;
                tokens_in: number;
                tokens_out: number;
            }[];
            total: number;
            distinct_channels: number;
            last_interaction_at: string;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=diary.d.ts.map