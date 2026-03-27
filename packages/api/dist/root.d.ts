import type { inferRouterInputs, inferRouterOutputs } from "@trpc/server";
/**
 * Root tRPC router merging all 10 domain routers.
 *
 * Dashboard-local routers (cc-session, resolve) are merged at the
 * catch-all handler in apps/dashboard, not here.
 */
export declare const appRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("./trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    obligation: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        execute: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
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
        approve: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
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
    contact: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        list: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                relationship?: string | undefined;
                q?: string | undefined;
            };
            output: Record<string, unknown>[];
            meta: object;
        }>;
        getById: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        create: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                name: string;
                notes?: string | null | undefined;
                channel_ids?: Record<string, string> | undefined;
                relationship_type?: string | null | undefined;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        update: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                name?: string | undefined;
                notes?: string | null | undefined;
                channel_ids?: Record<string, string> | undefined;
                relationship_type?: string | null | undefined;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        delete: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        getRelated: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                id: string;
            };
            output: {
                contact: {
                    id: string;
                    name: string;
                    channel_ids: Record<string, string>;
                    relationship_type: string | null;
                    notes: string | null;
                    created_at: string;
                };
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
                }[];
                message_count: number;
                obligations: Record<string, unknown>[];
                memory_profile: string | null;
                channels_active: string[];
            };
            meta: object;
        }>;
        discovered: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                contacts: {
                    name: string;
                    channels: string[];
                    message_count: number;
                    first_seen: string;
                    last_seen: string;
                    contact_id: string | null;
                    relationship_type: string | null;
                    notes: string | null;
                    channel_ids: null;
                }[];
                total_senders: number;
                total_messages_scanned: number;
            };
            meta: object;
        }>;
        relationships: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                min_count?: number | undefined;
            };
            output: {
                relationships: {
                    person_a: string;
                    person_b: string;
                    shared_channel: string;
                    co_occurrence_count: number;
                }[];
            };
            meta: object;
        }>;
        resolve: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                senders: string[];
            };
            output: Record<string, string>;
            meta: object;
        }>;
    }>>;
    diary: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    briefing: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        latest: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                entry: null;
            } | {
                entry: {
                    id: string;
                    generated_at: string;
                    content: string;
                    suggested_actions: import("./routers/briefing.js").BriefingAction[];
                    sources_status: Record<string, string>;
                };
            };
            meta: object;
        }>;
        history: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                limit?: number | undefined;
            };
            output: {
                entries: {
                    id: string;
                    generated_at: string;
                    content: string;
                    suggested_actions: import("./routers/briefing.js").BriefingAction[];
                    sources_status: Record<string, string>;
                }[];
            };
            meta: object;
        }>;
        generate: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                success: boolean;
                briefing_id: string;
            };
            meta: object;
        }>;
    }>>;
    message: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    session: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    automation: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        getAll: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                reminders: {
                    id: string;
                    message: string;
                    due_at: string;
                    channel: string;
                    created_at: string;
                    status: "overdue" | "pending";
                }[];
                schedules: {
                    id: string;
                    name: string;
                    cron_expr: string;
                    action: string;
                    channel: string;
                    enabled: boolean;
                    last_run_at: string | null;
                    next_run: string | null;
                }[];
                watcher: {
                    enabled: boolean;
                    interval_minutes: number;
                    quiet_start: string;
                    quiet_end: string;
                    last_run_at: string | null;
                };
                briefing: {
                    last_generated_at: string;
                    content_preview: string | null;
                    briefing_hour: number;
                    next_generation: string;
                };
                active_sessions: {
                    id: string;
                    project: string;
                    command: string;
                    status: string;
                    started_at: string;
                }[];
            };
            meta: object;
        }>;
        listReminders: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                id: string;
                message: string;
                due_at: string;
                channel: string;
                created_at: string;
                status: "overdue" | "pending";
            }[];
            meta: object;
        }>;
        updateReminder: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                action: "cancel";
            };
            output: {
                id: string;
                message: string;
                due_at: string;
                channel: string;
                created_at: string;
                cancelled: boolean;
            };
            meta: object;
        }>;
        listSchedules: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                id: string;
                name: string;
                cron_expr: string;
                action: string;
                channel: string;
                enabled: boolean;
                last_run_at: string | null;
                next_run: string | null;
            }[];
            meta: object;
        }>;
        updateSchedule: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                enabled: boolean;
            };
            output: {
                id: string;
                name: string;
                cron_expr: string;
                action: string;
                channel: string;
                enabled: boolean;
                last_run_at: string | null;
            };
            meta: object;
        }>;
        getSettings: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                settings: Record<string, string>;
            };
            meta: object;
        }>;
        updateSettings: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                key: string;
                value: string;
            };
            output: {
                key: string;
                value: string;
                updatedAt: Date;
            };
            meta: object;
        }>;
        getWatcher: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                enabled: boolean;
                interval_minutes: number;
                quiet_start: string;
                quiet_end: string;
                last_run_at: string | null;
            };
            meta: object;
        }>;
        updateWatcher: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                enabled?: boolean | undefined;
                interval_minutes?: number | undefined;
                quiet_start?: string | undefined;
                quiet_end?: string | undefined;
            };
            output: {
                enabled: boolean;
                interval_minutes: number;
                quiet_start: string;
                quiet_end: string;
                last_run_at: string | null;
            };
            meta: object;
        }>;
    }>>;
    system: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        latency: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                services: {};
                timestamp: string;
                note: string;
            };
            meta: object;
        }>;
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
    auth: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        verify: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                token?: string | undefined;
            };
            output: {
                ok: boolean;
                error?: undefined;
            } | {
                ok: boolean;
                error: string;
            };
            meta: object;
        }>;
        logout: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                ok: boolean;
            };
            meta: object;
        }>;
    }>>;
    project: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        extract: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                projects_updated: number;
                sources_scanned: string[];
            };
            meta: object;
        }>;
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
}>>;
export type AppRouter = typeof appRouter;
/** Type helper for inferring procedure return types from the router. */
export type RouterOutputs = inferRouterOutputs<AppRouter>;
/** Type helper for inferring procedure input types from the router. */
export type RouterInputs = inferRouterInputs<AppRouter>;
/** Server-side caller factory. */
export declare const createCaller: import("@trpc/server").TRPCRouterCaller<{
    ctx: import("./trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    obligation: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        execute: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
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
        approve: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
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
    contact: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        list: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                relationship?: string | undefined;
                q?: string | undefined;
            };
            output: Record<string, unknown>[];
            meta: object;
        }>;
        getById: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        create: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                name: string;
                notes?: string | null | undefined;
                channel_ids?: Record<string, string> | undefined;
                relationship_type?: string | null | undefined;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        update: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                name?: string | undefined;
                notes?: string | null | undefined;
                channel_ids?: Record<string, string> | undefined;
                relationship_type?: string | null | undefined;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        delete: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
            };
            output: Record<string, unknown>;
            meta: object;
        }>;
        getRelated: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                id: string;
            };
            output: {
                contact: {
                    id: string;
                    name: string;
                    channel_ids: Record<string, string>;
                    relationship_type: string | null;
                    notes: string | null;
                    created_at: string;
                };
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
                }[];
                message_count: number;
                obligations: Record<string, unknown>[];
                memory_profile: string | null;
                channels_active: string[];
            };
            meta: object;
        }>;
        discovered: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                contacts: {
                    name: string;
                    channels: string[];
                    message_count: number;
                    first_seen: string;
                    last_seen: string;
                    contact_id: string | null;
                    relationship_type: string | null;
                    notes: string | null;
                    channel_ids: null;
                }[];
                total_senders: number;
                total_messages_scanned: number;
            };
            meta: object;
        }>;
        relationships: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                min_count?: number | undefined;
            };
            output: {
                relationships: {
                    person_a: string;
                    person_b: string;
                    shared_channel: string;
                    co_occurrence_count: number;
                }[];
            };
            meta: object;
        }>;
        resolve: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                senders: string[];
            };
            output: Record<string, string>;
            meta: object;
        }>;
    }>>;
    diary: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    briefing: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        latest: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                entry: null;
            } | {
                entry: {
                    id: string;
                    generated_at: string;
                    content: string;
                    suggested_actions: import("./routers/briefing.js").BriefingAction[];
                    sources_status: Record<string, string>;
                };
            };
            meta: object;
        }>;
        history: import("@trpc/server").TRPCQueryProcedure<{
            input: {
                limit?: number | undefined;
            };
            output: {
                entries: {
                    id: string;
                    generated_at: string;
                    content: string;
                    suggested_actions: import("./routers/briefing.js").BriefingAction[];
                    sources_status: Record<string, string>;
                }[];
            };
            meta: object;
        }>;
        generate: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                success: boolean;
                briefing_id: string;
            };
            meta: object;
        }>;
    }>>;
    message: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    session: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
    automation: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        getAll: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                reminders: {
                    id: string;
                    message: string;
                    due_at: string;
                    channel: string;
                    created_at: string;
                    status: "overdue" | "pending";
                }[];
                schedules: {
                    id: string;
                    name: string;
                    cron_expr: string;
                    action: string;
                    channel: string;
                    enabled: boolean;
                    last_run_at: string | null;
                    next_run: string | null;
                }[];
                watcher: {
                    enabled: boolean;
                    interval_minutes: number;
                    quiet_start: string;
                    quiet_end: string;
                    last_run_at: string | null;
                };
                briefing: {
                    last_generated_at: string;
                    content_preview: string | null;
                    briefing_hour: number;
                    next_generation: string;
                };
                active_sessions: {
                    id: string;
                    project: string;
                    command: string;
                    status: string;
                    started_at: string;
                }[];
            };
            meta: object;
        }>;
        listReminders: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                id: string;
                message: string;
                due_at: string;
                channel: string;
                created_at: string;
                status: "overdue" | "pending";
            }[];
            meta: object;
        }>;
        updateReminder: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                action: "cancel";
            };
            output: {
                id: string;
                message: string;
                due_at: string;
                channel: string;
                created_at: string;
                cancelled: boolean;
            };
            meta: object;
        }>;
        listSchedules: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                id: string;
                name: string;
                cron_expr: string;
                action: string;
                channel: string;
                enabled: boolean;
                last_run_at: string | null;
                next_run: string | null;
            }[];
            meta: object;
        }>;
        updateSchedule: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                id: string;
                enabled: boolean;
            };
            output: {
                id: string;
                name: string;
                cron_expr: string;
                action: string;
                channel: string;
                enabled: boolean;
                last_run_at: string | null;
            };
            meta: object;
        }>;
        getSettings: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                settings: Record<string, string>;
            };
            meta: object;
        }>;
        updateSettings: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                key: string;
                value: string;
            };
            output: {
                key: string;
                value: string;
                updatedAt: Date;
            };
            meta: object;
        }>;
        getWatcher: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                enabled: boolean;
                interval_minutes: number;
                quiet_start: string;
                quiet_end: string;
                last_run_at: string | null;
            };
            meta: object;
        }>;
        updateWatcher: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                enabled?: boolean | undefined;
                interval_minutes?: number | undefined;
                quiet_start?: string | undefined;
                quiet_end?: string | undefined;
            };
            output: {
                enabled: boolean;
                interval_minutes: number;
                quiet_start: string;
                quiet_end: string;
                last_run_at: string | null;
            };
            meta: object;
        }>;
    }>>;
    system: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        latency: import("@trpc/server").TRPCQueryProcedure<{
            input: void;
            output: {
                services: {};
                timestamp: string;
                note: string;
            };
            meta: object;
        }>;
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
    auth: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
        verify: import("@trpc/server").TRPCMutationProcedure<{
            input: {
                token?: string | undefined;
            };
            output: {
                ok: boolean;
                error?: undefined;
            } | {
                ok: boolean;
                error: string;
            };
            meta: object;
        }>;
        logout: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                ok: boolean;
            };
            meta: object;
        }>;
    }>>;
    project: import("@trpc/server").TRPCBuiltRouter<{
        ctx: import("./trpc.js").TRPCContext;
        meta: object;
        errorShape: import("@trpc/server").TRPCDefaultErrorShape;
        transformer: true;
    }, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
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
        extract: import("@trpc/server").TRPCMutationProcedure<{
            input: void;
            output: {
                projects_updated: number;
                sources_scanned: string[];
            };
            meta: object;
        }>;
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
}>>;
//# sourceMappingURL=root.d.ts.map