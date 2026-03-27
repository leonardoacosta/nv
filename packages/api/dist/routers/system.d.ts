export declare const systemRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * Health check (DB ping).
     */
    health: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            daemon: {
                database: {
                    status: string;
                    error?: undefined;
                };
                note: string;
            };
            latest: null;
            status: string;
            history: never[];
        } | {
            daemon: {
                database: {
                    status: string;
                    error: string;
                };
                note?: undefined;
            };
            latest: null;
            status: string;
            history: never[];
        };
        meta: object;
    }>;
    /**
     * Latency monitoring (placeholder -- meta-svc handles this on the host).
     */
    latency: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            services: {};
            timestamp: string;
            note: string;
        };
        meta: object;
    }>;
    /**
     * Stats: entity counts across tables.
     */
    stats: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            tool_usage: {
                total_invocations: number;
                invocations_today: number;
                per_tool: {
                    tool: string;
                    count: number;
                }[];
            };
            counts: {
                messages: number;
                obligations: number;
                contacts: number;
                memory: number;
                diary: number;
            };
        };
        meta: object;
    }>;
    /**
     * Fleet service registry (static, no HTTP calls).
     */
    fleetStatus: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            fleet: {
                status: string;
                services: {
                    status: "unknown";
                    latency_ms: number | null;
                    name: string;
                    url: string;
                    port: number;
                    tools: string[];
                }[];
                healthy_count: number;
                total_count: number;
            };
            channels: {
                name: string;
                status: "configured";
                direction: "bidirectional";
            }[];
        };
        meta: object;
    }>;
    /**
     * Activity feed: merged timeline from messages, obligations, diary, sessions (last 24h).
     */
    activityFeed: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            events: {
                id: string;
                type: string;
                timestamp: string;
                icon_hint: string;
                summary: string;
                severity: "error" | "warning" | "info";
            }[];
        };
        meta: object;
    }>;
    /**
     * Config: fleet service URLs and project config from env.
     */
    config: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            tool_router_url: string;
            memory_svc_url: string;
            messages_svc_url: string;
            meta_svc_url: string;
            nv_projects: string;
        };
        meta: object;
    }>;
    /**
     * Memory: get topic or list of topics.
     */
    memory: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            topic?: string | undefined;
        };
        output: {
            topic: string;
            content: string;
            topics?: undefined;
        } | {
            topics: string[];
            topic?: undefined;
            content?: undefined;
        };
        meta: object;
    }>;
    /**
     * Memory: upsert a topic.
     */
    updateMemory: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            content: string;
            topic: string;
        };
        output: {
            topic: string;
            written: number;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=system.d.ts.map