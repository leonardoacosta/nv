export declare const automationRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * Get full automations overview (reminders, schedules, watcher, briefing, active sessions).
     */
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
    /**
     * Create a new reminder.
     */
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
    /**
     * Cancel a reminder by ID.
     */
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
    /**
     * List all schedules.
     */
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
    /**
     * Toggle a schedule enabled/disabled.
     */
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
    /**
     * Get all settings.
     */
    getSettings: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            settings: Record<string, string>;
        };
        meta: object;
    }>;
    /**
     * Update a setting (upsert by key).
     */
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
    /**
     * Get watcher config and update it.
     */
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
    /**
     * Update watcher configuration (in-memory, reverts on restart).
     */
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
    /**
     * Assemble a preview of the prompt context that would be sent to Nova for a
     * given automation type (watcher | briefing).
     *
     * Queries obligations, memory, and messages with a 5-second per-source
     * timeout via Promise.allSettled. Each section reports its own status
     * (ok / unavailable / empty) so the UI can surface partial failures.
     */
    previewContext: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            type: "watcher" | "briefing";
        };
        output: {
            obligations: {
                status: "ok" | "unavailable" | "empty";
                items: {
                    id: string;
                    detectedAction: string;
                    status: string;
                    priority: number;
                    sourceChannel: string;
                    deadline: string | null;
                    createdAt: string;
                }[];
                countByStatus: Record<string, number>;
            };
            memory: {
                status: "ok" | "unavailable" | "empty";
                items: {
                    topic: string;
                    contentPreview: string;
                }[];
            };
            messages: {
                status: "ok" | "unavailable" | "empty";
                byChannel: {
                    channel: string;
                    count: number;
                    latestPreview: string | null;
                }[];
            };
            channels: {
                name: string;
                messageCount: number;
                active: boolean;
            }[];
            stats: {
                totalObligations: number;
                activeReminders: number;
                memoryTopics: number;
            };
            assembledAt: string;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=automation.d.ts.map