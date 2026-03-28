export interface ToolCallDetail {
    name: string;
    input_summary: string;
    duration_ms: number | null;
}
export declare const diaryRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List diary entries with optional date/limit filters.
     * Returns normalized entries with both legacy and new tool formats unified,
     * plus daily aggregate statistics.
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
                tools_detail: ToolCallDetail[];
                result_summary: string;
                response_latency_ms: number;
                tokens_in: number;
                tokens_out: number;
                model: string | null;
                cost_usd: number | null;
            }[];
            total: number;
            distinct_channels: number;
            last_interaction_at: string;
            aggregates: {
                total_tokens_in: number;
                total_tokens_out: number;
                total_cost_usd: number | null;
                avg_latency_ms: number;
                tool_frequency: {
                    name: string;
                    count: number;
                }[];
            };
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=diary.d.ts.map