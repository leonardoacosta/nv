export declare const sessionRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List sessions with pagination and filters.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
            project?: string | undefined;
            trigger_type?: string | undefined;
            page?: number | undefined;
            date_from?: string | undefined;
            date_to?: string | undefined;
        };
        output: {
            sessions: {
                id: string;
                project: string;
                command: string;
                status: string;
                trigger_type: string | null;
                message_count: number;
                tool_count: number;
                started_at: string;
                stopped_at: string | null;
                duration_display: string;
            }[];
            total: number;
            page: number;
            limit: number;
        };
        meta: object;
    }>;
    /**
     * Get a single session by ID.
     */
    getById: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            id: string;
        };
        output: {
            id: string;
            service: string;
            status: string;
            messages: number;
            tools_executed: number;
            started_at: string;
            ended_at: string | null;
            project: string;
            trigger_type: string | null;
            message_count: number;
            tool_count: number;
        };
        meta: object;
    }>;
    /**
     * Get aggregated session analytics.
     */
    analytics: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            sessions_today: number;
            sessions_7d: {
                date: string;
                count: number;
            }[];
            avg_duration_mins: number;
            project_breakdown: {
                project: string;
                count: number;
            }[];
            total_sessions: number;
        };
        meta: object;
    }>;
    /**
     * Get events for a session.
     */
    getEvents: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            id: string;
        };
        output: {
            events: {
                id: string;
                session_id: string;
                event_type: string;
                direction: string | null;
                content: string | null;
                metadata: Record<string, unknown> | null;
                created_at: string;
            }[];
        };
        meta: object;
    }>;
    /**
     * Get CC-type sessions (identified by "claude" in command).
     */
    ccSessions: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            sessions: {
                id: string;
                project: string;
                state: string;
                machine_name: string;
                started_at: string;
                duration_display: string;
                restart_attempts: number;
            }[];
            configured: boolean;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=session.d.ts.map