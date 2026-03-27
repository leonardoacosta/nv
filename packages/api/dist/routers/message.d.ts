export declare const messageRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List messages with channel/direction/sort/type/limit/offset filters.
     * Returns { messages, total, limit, offset }.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            type?: "conversation" | "tool-call" | "system" | undefined;
            sort?: "asc" | "desc" | undefined;
            limit?: number | undefined;
            offset?: number | undefined;
            channel?: string | undefined;
            direction?: "outbound" | "inbound" | undefined;
        };
        output: {
            messages: {
                id: number;
                timestamp: string;
                direction: string;
                channel: string;
                sender: string;
                content: string;
                response_time_ms: null;
                tokens_in: null;
                tokens_out: null;
                type: "conversation" | "tool-call" | "system";
            }[];
            total: number;
            limit: number;
            offset: number;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=message.d.ts.map