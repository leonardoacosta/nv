export interface ConfigSourceEntry {
    key: string;
    source: "env" | "file" | "default";
    envVar?: string;
}
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
     * Fleet service status — calls meta-svc /services for live health data.
     * Falls back to static registry with status "unknown" if meta-svc is unreachable.
     * Also inserts health snapshots for historical uptime tracking.
     */
    fleetStatus: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            fleet: {
                status: "unknown" | "healthy" | "degraded";
                services: {
                    name: string;
                    url: string;
                    port: number;
                    status: "healthy" | "unreachable" | "unknown";
                    latency_ms: number | null;
                    tools: string[];
                    last_checked: string | null;
                    uptime_secs: number | null;
                }[];
                healthy_count: number;
                total_count: number;
            };
            channels: {
                name: string;
                status: "configured" | "unknown" | "connected" | "disconnected" | "unconfigured";
                direction: "bidirectional" | "inbound" | "outbound";
                messages_24h: number | null;
                messages_per_hour: number | null;
            }[];
        };
        meta: object;
    }>;
    /**
     * Channel volume: message counts per channel over the last 24h, bucketed by hour.
     */
    channelVolume: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            channels: {
                total_24h: number;
                hourly: {
                    hour: string;
                    count: number;
                }[];
                name: string;
            }[];
        };
        meta: object;
    }>;
    /**
     * Error rates: session_events with error/tool_error types in last 24h,
     * grouped by hour and event type.
     */
    errorRates: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            total_24h: number;
            hourly: {
                hour: string;
                count: number;
            }[];
            by_type: {
                event_type: string;
                count: number;
            }[];
        };
        meta: object;
    }>;
    /**
     * Fleet history: last 24h of fleet health snapshots, downsampled to 15-min buckets.
     * Returns uptime percentage per service.
     */
    fleetHistory: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            services: {
                name: string;
                snapshots: {
                    time: string;
                    status: string;
                    latency_ms: number | null;
                }[];
                uptime_pct_24h: number;
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
    /**
     * Config sources: proxy GET /config/sources from the Rust daemon.
     * Returns which env var, TOML file, or default resolved each config key.
     * Falls back to an empty array if the daemon endpoint is not yet available.
     */
    configSources: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: ConfigSourceEntry[];
        meta: object;
    }>;
    /**
     * Channel status: call channels-svc /channels for live connection status
     * and enrich each channel with lastMessageAt from the messages table.
     * Falls back to the static registry on error.
     */
    channelStatus: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            name: string;
            connected: boolean;
            error: string | null;
            identity: {
                username?: string;
                displayName?: string;
            } | null;
            lastMessageAt: string | null;
        }[];
        meta: object;
    }>;
    /**
     * Test channel: send a test message via channels-svc /send.
     */
    testChannel: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            channel: string;
            target: string;
        };
        output: {
            valid: boolean;
            error: null;
            latencyMs: number;
        } | {
            valid: boolean;
            error: string;
            latencyMs: number;
        };
        meta: object;
    }>;
    /**
     * Test integration: validate an external service API key by making a
     * lightweight server-side request. Keys are never sent to the browser.
     */
    testIntegration: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            service: "anthropic" | "openai" | "elevenlabs" | "github" | "sentry" | "posthog";
        };
        output: {
            valid: boolean;
            error: string;
            latencyMs: number;
        } | {
            valid: boolean;
            error: null;
            latencyMs: number;
        };
        meta: object;
    }>;
    /**
     * Memory summary: returns topic count, topic names, last write timestamp,
     * and total content size in bytes.
     */
    memorySummary: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            count: number;
            topics: string[];
            lastWriteAt: string | null;
            totalSizeBytes: number;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=system.d.ts.map