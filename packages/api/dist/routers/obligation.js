import { and, count, desc, eq, gte, like, lte, ne, sql } from "drizzle-orm";
import { z } from "zod";
import { db } from "@nova/db";
import { obligations, messages, reminders, sessions, } from "@nova/db";
import { createTRPCRouter, protectedProcedure } from "../trpc.js";
import { TRPCError } from "@trpc/server";
/** Shallow convert camelCase keys to snake_case, Date -> ISO string. */
function toSnakeCase(obj) {
    const result = {};
    for (const [key, value] of Object.entries(obj)) {
        const snakeKey = key.replace(/[A-Z]/g, (l) => `_${l.toLowerCase()}`);
        result[snakeKey] = value instanceof Date ? value.toISOString() : value;
    }
    return result;
}
export const obligationRouter = createTRPCRouter({
    /**
     * List obligations with optional status/owner filters.
     * Returns { obligations: [...] } matching the current API shape.
     */
    list: protectedProcedure
        .input(z.object({
        status: z.string().optional(),
        owner: z.string().optional(),
    }))
        .query(async ({ input }) => {
        const conditions = [];
        if (input.status)
            conditions.push(eq(obligations.status, input.status));
        if (input.owner)
            conditions.push(eq(obligations.owner, input.owner));
        const where = conditions.length === 0
            ? undefined
            : conditions.length === 1
                ? conditions[0]
                : and(...conditions);
        const rows = await db
            .select()
            .from(obligations)
            .where(where)
            .orderBy(desc(obligations.createdAt));
        const mapped = rows.map((row) => ({
            ...toSnakeCase(row),
            notes: [],
            attempt_count: 0,
        }));
        return { obligations: mapped };
    }),
    /**
     * Get a single obligation by ID.
     */
    getById: protectedProcedure
        .input(z.object({ id: z.string().uuid() }))
        .query(async ({ input }) => {
        const [row] = await db
            .select()
            .from(obligations)
            .where(eq(obligations.id, input.id))
            .limit(1);
        if (!row) {
            throw new TRPCError({ code: "NOT_FOUND", message: "Obligation not found" });
        }
        return {
            ...toSnakeCase(row),
            notes: [],
            attempt_count: 0,
        };
    }),
    /**
     * Create a new obligation.
     */
    create: protectedProcedure
        .input(z.object({
        detected_action: z.string().min(1),
        owner: z.string().default("nova"),
        status: z.string().default("open"),
        priority: z.number().int().min(0).max(4).default(2),
        source_channel: z.string().default("dashboard"),
    }))
        .mutation(async ({ input }) => {
        const [row] = await db
            .insert(obligations)
            .values({
            detectedAction: input.detected_action.trim(),
            owner: input.owner,
            status: input.status,
            priority: input.priority,
            sourceChannel: input.source_channel,
        })
            .returning({ id: obligations.id });
        if (!row) {
            throw new TRPCError({ code: "INTERNAL_SERVER_ERROR", message: "Failed to create obligation" });
        }
        return { obligation: { id: row.id } };
    }),
    /**
     * Update an obligation by ID. Accepts snake_case fields.
     */
    update: protectedProcedure
        .input(z.object({
        id: z.string().uuid(),
        status: z.string().optional(),
        owner: z.string().optional(),
        priority: z.number().int().min(0).max(4).optional(),
        detected_action: z.string().optional(),
        project_code: z.string().optional(),
        deadline: z.string().optional(),
    }))
        .mutation(async ({ input }) => {
        const updates = {};
        if (input.status !== undefined)
            updates.status = input.status;
        if (input.owner !== undefined)
            updates.owner = input.owner;
        if (input.priority !== undefined)
            updates.priority = input.priority;
        if (input.detected_action !== undefined)
            updates.detectedAction = input.detected_action;
        if (input.project_code !== undefined)
            updates.projectCode = input.project_code;
        if (input.deadline !== undefined)
            updates.deadline = input.deadline;
        updates.updatedAt = new Date();
        const [updated] = await db
            .update(obligations)
            .set(updates)
            .where(eq(obligations.id, input.id))
            .returning();
        if (!updated) {
            throw new TRPCError({ code: "NOT_FOUND", message: "Obligation not found" });
        }
        return toSnakeCase(updated);
    }),
    /**
     * Execute an obligation (set status to in_progress).
     */
    execute: protectedProcedure
        .input(z.object({ id: z.string().uuid() }))
        .mutation(async ({ input }) => {
        const [updated] = await db
            .update(obligations)
            .set({
            status: "in_progress",
            lastAttemptAt: new Date(),
            updatedAt: new Date(),
        })
            .where(eq(obligations.id, input.id))
            .returning();
        if (!updated) {
            throw new TRPCError({ code: "NOT_FOUND", message: "Obligation not found" });
        }
        return toSnakeCase(updated);
    }),
    /**
     * Get recent obligation activity (status changes).
     */
    activity: protectedProcedure
        .input(z.object({ limit: z.number().int().min(1).max(100).default(20) }))
        .query(async ({ input }) => {
        const rows = await db
            .select()
            .from(obligations)
            .orderBy(desc(obligations.updatedAt))
            .limit(input.limit);
        const events = rows.map((row) => ({
            id: row.id,
            event_type: "status_change",
            obligation_id: row.id,
            description: `${row.detectedAction} — ${row.status}`,
            timestamp: row.updatedAt.toISOString(),
        }));
        return { events };
    }),
    /**
     * Get obligation stats (counts by status/owner).
     */
    stats: protectedProcedure.query(async () => {
        const statusCounts = await db
            .select({
            status: obligations.status,
            owner: obligations.owner,
            count: sql `count(*)::int`,
        })
            .from(obligations)
            .groupBy(obligations.status, obligations.owner);
        let openNova = 0;
        let openLeo = 0;
        let inProgress = 0;
        let proposedDone = 0;
        for (const row of statusCounts) {
            if (row.status === "open" && row.owner === "nova")
                openNova += row.count;
            if (row.status === "open" && row.owner === "leo")
                openLeo += row.count;
            if (row.status === "in_progress")
                inProgress += row.count;
            if (row.status === "proposed_done")
                proposedDone += row.count;
        }
        const todayStart = new Date();
        todayStart.setHours(0, 0, 0, 0);
        const [doneTodayResult] = await db
            .select({ count: sql `count(*)::int` })
            .from(obligations)
            .where(and(eq(obligations.status, "done"), gte(obligations.updatedAt, todayStart)));
        return {
            open_nova: openNova,
            open_leo: openLeo,
            in_progress: inProgress,
            proposed_done: proposedDone,
            done_today: doneTodayResult?.count ?? 0,
        };
    }),
    /**
     * Approve an obligation (set status to proposed_done).
     */
    approve: protectedProcedure
        .input(z.object({ id: z.string().uuid() }))
        .mutation(async ({ input }) => {
        const [updated] = await db
            .update(obligations)
            .set({
            status: "proposed_done",
            updatedAt: new Date(),
        })
            .where(eq(obligations.id, input.id))
            .returning();
        if (!updated) {
            throw new TRPCError({ code: "NOT_FOUND", message: "Obligation not found" });
        }
        return toSnakeCase(updated);
    }),
    /**
     * Get related entities for an obligation (source message, project context, reminders, related obligations).
     */
    getRelated: protectedProcedure
        .input(z.object({ id: z.string().uuid() }))
        .query(async ({ input }) => {
        const [obligation] = await db
            .select()
            .from(obligations)
            .where(eq(obligations.id, input.id))
            .limit(1);
        if (!obligation) {
            throw new TRPCError({ code: "NOT_FOUND", message: "Obligation not found" });
        }
        const ONE_HOUR_MS = 3_600_000;
        // Find source message via content + time window matching
        let sourceMessage = null;
        if (obligation.sourceMessage && obligation.sourceChannel) {
            const snippet = obligation.sourceMessage.slice(0, 100);
            const windowStart = new Date(obligation.createdAt.getTime() - ONE_HOUR_MS);
            const windowEnd = new Date(obligation.createdAt.getTime() + ONE_HOUR_MS);
            const [sourceRow] = await db
                .select()
                .from(messages)
                .where(and(eq(messages.channel, obligation.sourceChannel), like(messages.content, `%${snippet}%`), gte(messages.createdAt, windowStart), lte(messages.createdAt, windowEnd)))
                .limit(1);
            if (sourceRow) {
                sourceMessage = {
                    id: 0,
                    timestamp: sourceRow.createdAt.toISOString(),
                    direction: sourceRow.sender === "nova" ? "outbound" : "inbound",
                    channel: sourceRow.channel ?? "unknown",
                    sender: sourceRow.sender ?? "unknown",
                    content: sourceRow.content,
                    response_time_ms: null,
                    tokens_in: null,
                    tokens_out: null,
                };
            }
        }
        // Project context
        let projectContext = null;
        if (obligation.projectCode) {
            const [oblCount] = await db
                .select({ total: count() })
                .from(obligations)
                .where(eq(obligations.projectCode, obligation.projectCode));
            const [sessCount] = await db
                .select({ total: count() })
                .from(sessions)
                .where(eq(sessions.project, obligation.projectCode));
            projectContext = {
                code: obligation.projectCode,
                obligation_count: oblCount?.total ?? 0,
                session_count: sessCount?.total ?? 0,
            };
        }
        // Reminders for this obligation
        const reminderRows = await db
            .select()
            .from(reminders)
            .where(eq(reminders.obligationId, obligation.id));
        const mappedReminders = reminderRows.map((r) => ({
            id: r.id,
            message: r.message,
            due_at: r.dueAt.toISOString(),
            status: r.cancelled
                ? "cancelled"
                : r.deliveredAt
                    ? "delivered"
                    : "pending",
        }));
        // Related obligations in the same project
        let relatedObligations = [];
        if (obligation.projectCode) {
            const relatedRows = await db
                .select()
                .from(obligations)
                .where(and(eq(obligations.projectCode, obligation.projectCode), ne(obligations.id, obligation.id)))
                .orderBy(desc(obligations.createdAt))
                .limit(10);
            relatedObligations = relatedRows.map((row) => ({
                ...toSnakeCase(row),
                notes: [],
                attempt_count: 0,
            }));
        }
        const mappedObligation = {
            ...toSnakeCase(obligation),
            notes: [],
            attempt_count: 0,
        };
        return {
            obligation: mappedObligation,
            source_message: sourceMessage,
            project: projectContext,
            reminders: mappedReminders,
            related_obligations: relatedObligations,
        };
    }),
});
//# sourceMappingURL=obligation.js.map