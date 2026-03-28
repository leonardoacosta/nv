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
                senderResolved: {
                    displayName: string;
                    avatarInitial: string;
                    source: "contact" | "telegram-meta" | "memory" | "raw";
                };
            }[];
            total: number;
            limit: number;
            offset: number;
        };
        meta: object;
    }>;
    /**
     * Cursor-based pagination for the chat history page.
     * Returns messages in reverse chronological order (newest first) so the
     * UI can use `flex-col-reverse` and load older pages upward.
     *
     * Filters to conversation-type messages only (excludes tool-call/system).
     * When `cursor` (ISO datetime) is provided, returns messages older than it.
     * Returns `nextCursor` (oldest row's createdAt ISO) when more pages exist.
     */
    chatHistory: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
            cursor?: string | undefined;
        };
        output: {
            messages: {
                id: number;
                timestamp: string;
                direction: string;
                channel: string;
                sender: string;
                content: string;
                response_time_ms: number | null;
                tokens_in: number | null;
                tokens_out: number | null;
                type: "conversation" | "tool-call" | "system";
            }[];
            nextCursor: string | null;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=message.d.ts.map