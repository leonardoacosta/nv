export interface BriefingAction {
    label: string;
    action: string;
    priority?: string;
}
export declare const briefingRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * Get the latest briefing with missedToday flag.
     */
    latest: import("@trpc/server").TRPCQueryProcedure<{
        input: void;
        output: {
            entry: null;
            missedToday: boolean;
        } | {
            entry: {
                id: string;
                generated_at: string;
                content: string;
                suggested_actions: BriefingAction[];
                sources_status: Record<string, string>;
                blocks: ({
                    type: "section";
                    data: {
                        body: string;
                    };
                    title?: string | undefined;
                } | {
                    type: "status_table";
                    data: {
                        columns: string[];
                        rows: Record<string, string>[];
                    };
                    title?: string | undefined;
                } | {
                    type: "metric_card";
                    data: {
                        value: string | number;
                        label: string;
                        unit?: string | undefined;
                        trend?: "flat" | "up" | "down" | undefined;
                        delta?: string | undefined;
                    };
                    title?: string | undefined;
                } | {
                    type: "timeline";
                    data: {
                        events: {
                            time: string;
                            label: string;
                            detail?: string | undefined;
                            severity?: "info" | "warning" | "error" | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "action_group";
                    data: {
                        actions: {
                            label: string;
                            status?: "pending" | "completed" | "dismissed" | undefined;
                            url?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "kv_list";
                    data: {
                        items: {
                            value: string;
                            key: string;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "alert";
                    data: {
                        message: string;
                        severity: "info" | "warning" | "error";
                    };
                    title?: string | undefined;
                } | {
                    type: "source_pills";
                    data: {
                        sources: {
                            status: "ok" | "unavailable" | "empty";
                            name: string;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "pr_list";
                    data: {
                        prs: {
                            status: "open" | "merged" | "closed";
                            title: string;
                            repo: string;
                            url?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "pipeline_table";
                    data: {
                        pipelines: {
                            status: "pending" | "success" | "failed" | "running";
                            name: string;
                            duration?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                })[] | null;
            };
            missedToday: boolean;
        };
        meta: object;
    }>;
    /**
     * Get briefing history.
     */
    history: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            limit?: number | undefined;
        };
        output: {
            entries: {
                id: string;
                generated_at: string;
                content: string;
                suggested_actions: BriefingAction[];
                sources_status: Record<string, string>;
                blocks: ({
                    type: "section";
                    data: {
                        body: string;
                    };
                    title?: string | undefined;
                } | {
                    type: "status_table";
                    data: {
                        columns: string[];
                        rows: Record<string, string>[];
                    };
                    title?: string | undefined;
                } | {
                    type: "metric_card";
                    data: {
                        value: string | number;
                        label: string;
                        unit?: string | undefined;
                        trend?: "flat" | "up" | "down" | undefined;
                        delta?: string | undefined;
                    };
                    title?: string | undefined;
                } | {
                    type: "timeline";
                    data: {
                        events: {
                            time: string;
                            label: string;
                            detail?: string | undefined;
                            severity?: "info" | "warning" | "error" | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "action_group";
                    data: {
                        actions: {
                            label: string;
                            status?: "pending" | "completed" | "dismissed" | undefined;
                            url?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "kv_list";
                    data: {
                        items: {
                            value: string;
                            key: string;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "alert";
                    data: {
                        message: string;
                        severity: "info" | "warning" | "error";
                    };
                    title?: string | undefined;
                } | {
                    type: "source_pills";
                    data: {
                        sources: {
                            status: "ok" | "unavailable" | "empty";
                            name: string;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "pr_list";
                    data: {
                        prs: {
                            status: "open" | "merged" | "closed";
                            title: string;
                            repo: string;
                            url?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                } | {
                    type: "pipeline_table";
                    data: {
                        pipelines: {
                            status: "pending" | "success" | "failed" | "running";
                            name: string;
                            duration?: string | undefined;
                        }[];
                    };
                    title?: string | undefined;
                })[] | null;
            }[];
        };
        meta: object;
    }>;
    /**
     * Trigger briefing generation via the daemon.
     */
    generate: import("@trpc/server").TRPCMutationProcedure<{
        input: void;
        output: {
            success: boolean;
            briefing_id: string;
        };
        meta: object;
    }>;
}>>;
//# sourceMappingURL=briefing.d.ts.map