export declare const contactRouter: import("@trpc/server").TRPCBuiltRouter<{
    ctx: import("../trpc.js").TRPCContext;
    meta: object;
    errorShape: import("@trpc/server").TRPCDefaultErrorShape;
    transformer: true;
}, import("@trpc/server").TRPCDecorateCreateRouterOptions<{
    /**
     * List contacts with optional relationship/q filters.
     */
    list: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            relationship?: string | undefined;
            q?: string | undefined;
        };
        output: Record<string, unknown>[];
        meta: object;
    }>;
    /**
     * Get a single contact by ID.
     */
    getById: import("@trpc/server").TRPCQueryProcedure<{
        input: {
            id: string;
        };
        output: Record<string, unknown>;
        meta: object;
    }>;
    /**
     * Create a new contact.
     */
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
    /**
     * Update a contact by ID.
     */
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
    /**
     * Delete a contact by ID.
     */
    delete: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            id: string;
        };
        output: Record<string, unknown>;
        meta: object;
    }>;
    /**
     * Get related data for a contact (messages, obligations, memory profile).
     */
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
    /**
     * Get discovered contacts from message data.
     */
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
    /**
     * Get sender relationship co-occurrences.
     */
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
    /**
     * Resolve sender identifiers to contact display names.
     */
    resolve: import("@trpc/server").TRPCMutationProcedure<{
        input: {
            senders: string[];
        };
        output: Record<string, string>;
        meta: object;
    }>;
}>>;
//# sourceMappingURL=contact.d.ts.map