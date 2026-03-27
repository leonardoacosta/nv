export declare const obligationRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List obligations with optional status/owner filters.
     * Returns { obligations: [...] } matching the current API shape.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            status?: string | undefined;
            owner?: string | undefined;
        };
        output: {
            obligations: {
                notes: never[];
                attempt_count: number;
            }[];
        };
        meta: object;
    }>;
    /**
     * Get a single obligation by ID.
     */
    getById: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            id: string;
        };
        output: {
            notes: never[];
            attempt_count: number;
        };
        meta: object;
    }>;
    /**
     * Create a new obligation.
     */
    create: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            detected_action: string;
            status?: string | undefined;
            owner?: string | undefined;
            priority?: number | undefined;
            source_channel?: string | undefined;
        };
        output: {
            obligation: {
                id: string;
            };
        };
        meta: object;
    }>;
    /**
     * Update an obligation by ID. Accepts snake_case fields.
     */
    update: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            id: string;
            status?: string | undefined;
            owner?: string | undefined;
            detected_action?: string | undefined;
            priority?: number | undefined;
            project_code?: string | undefined;
            deadline?: string | undefined;
        };
        output: Record<string, unknown>;
        meta: object;
    }>;
    /**
     * Execute an obligation (set status to in_progress).
     */
    execute: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            id: string;
        };
        output: Record<string, unknown>;
        meta: object;
    }>;
    /**
     * Get recent obligation activity (status changes).
     */
    activity: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
        };
        output: {
            events: {
                id: string;
                event_type: string;
                obligation_id: string;
                description: string;
                timestamp: string;
            }[];
        };
        meta: object;
    }>;
    /**
     * Get obligation stats (counts by status/owner).
     */
    stats: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            open_nova: number;
            open_leo: number;
            in_progress: number;
            proposed_done: number;
            done_today: number;
        };
        meta: object;
    }>;
    /**
     * Approve an obligation (set status to proposed_done).
     */
    approve: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            id: string;
        };
        output: Record<string, unknown>;
        meta: object;
    }>;
    /**
     * Get related entities for an obligation (source message, project context, reminders, related obligations).
     */
    getRelated: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            id: string;
        };
        output: {
            obligation: {
                notes: never[];
                attempt_count: number;
            };
            source_message: Record<string, unknown> | null;
            project: {
                code: string;
                obligation_count: number;
                session_count: number;
            } | null;
            reminders: {
                id: string;
                message: string;
                due_at: string;
                status: string;
            }[];
            related_obligations: Record<string, unknown>[];
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=obligation.d.ts.map