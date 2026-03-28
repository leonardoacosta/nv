export declare const projectRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List projects enriched with obligation/session counts.
     * Seeds from NV_PROJECTS env var if the table is empty.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            category?: string | undefined;
        };
        output: {
            projects: {
                id: string;
                code: string;
                name: string;
                category: string;
                status: string;
                description: string | null;
                content: string | null;
                path: string | null;
                obligation_count: number;
                active_obligation_count: number;
                session_count: number;
                last_activity: string | null;
                created_at: string;
                updated_at: string;
            }[];
        };
        meta: object;
    }>;
    /**
     * Get a project by its code.
     */
    getByCode: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            code: string;
        };
        output: {
            id: string;
            code: string;
            name: string;
            category: string;
            status: string;
            description: string | null;
            content: string | null;
            path: string | null;
            created_at: string;
            updated_at: string;
        };
        meta: object;
    }>;
    /**
     * Create a new project.
     */
    create: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            code: string;
            name: string;
            description?: string | undefined;
            path?: string | undefined;
            status?: string | undefined;
            content?: string | undefined;
            category?: string | undefined;
        };
        output: {
            id: string;
            code: string;
            name: string;
            category: string;
            status: string;
            description: string | null;
            content: string | null;
            path: string | null;
            obligation_count: number;
            active_obligation_count: number;
            session_count: number;
            last_activity: null;
            created_at: string;
            updated_at: string;
        };
        meta: object;
    }>;
    /**
     * Materialize projects from daemon registry and projects-* memory topics.
     * Upserts new projects and enriches existing ones with path/description.
     */
    materialize: import("@trpc/server").TRPCMutationProcedure<{
        input: void;
        output: import("../lib/materialize-projects.js").MaterializeResult;
        meta: object;
    }>;
    /**
     * Extract and assemble knowledge documents for all projects.
     */
    extract: import("@trpc/server").TRPCMutationProcedure<{
        input: void;
        output: {
            projects_updated: number;
            sources_scanned: string[];
        };
        meta: object;
    }>;
    /**
     * Get related entities for a project (obligations, sessions, memory, messages).
     */
    getRelated: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            code: string;
        };
        output: {
            project: {
                code: string;
                path: string;
            };
            obligations: {
                notes: never[];
                attempt_count: number;
            }[];
            obligation_summary: {
                total: number;
                open: number;
                in_progress: number;
                done: number;
            };
            sessions: {
                id: string;
                project: string;
                status: string;
                agent_name: string;
                started_at: string;
                duration_display: string;
                branch: undefined;
                spec: undefined;
                progress: undefined;
            }[];
            session_count: number;
            memory_topics: {
                topic: string;
                preview: string;
            }[];
            recent_messages: {
                id: number;
                timestamp: string;
                direction: string;
                channel: string;
                sender: string;
                content: string;
                response_time_ms: null;
                tokens_in: null;
                tokens_out: null;
            }[];
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=project.d.ts.map