import { and, asc, count, desc, eq, gte, like, lte, sql } from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";

import { db } from "@nova/db";
import { sessions, sessionEvents } from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";

function formatDuration(startedAt: Date, stoppedAt: Date | null): string {
  const end = stoppedAt ?? new Date();
  const diffMs = end.getTime() - startedAt.getTime();
  const totalSecs = Math.floor(diffMs / 1000);
  const hours = Math.floor(totalSecs / 3600);
  const mins = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;
  if (hours > 0) return `${hours}h ${mins}m`;
  if (mins > 0) return `${mins}m ${secs}s`;
  return `${secs}s`;
}

function deriveService(command: string): string {
  const lower = command.toLowerCase();
  if (lower.includes("claude")) return "CLI";
  if (lower.includes("telegram")) return "Telegram";
  return command;
}

function mapStatus(status: string): string {
  return status === "running" ? "active" : status;
}

export const sessionRouter = createTRPCRouter({
  /**
   * List sessions with pagination and filters.
   */
  list: protectedProcedure
    .input(
      z.object({
        page: z.number().int().min(1).default(1),
        limit: z.number().int().min(1).max(100).default(25),
        project: z.string().optional(),
        trigger_type: z.string().optional(),
        date_from: z.string().optional(),
        date_to: z.string().optional(),
      }),
    )
    .query(async ({ input }) => {
      const conditions = [];
      if (input.project) {
        conditions.push(eq(sessions.project, input.project));
      }
      if (input.trigger_type) {
        conditions.push(eq(sessions.triggerType, input.trigger_type));
      }
      if (input.date_from) {
        conditions.push(gte(sessions.startedAt, new Date(input.date_from)));
      }
      if (input.date_to) {
        const endDate = new Date(input.date_to);
        endDate.setDate(endDate.getDate() + 1);
        conditions.push(lte(sessions.startedAt, endDate));
      }

      const whereClause = conditions.length > 0 ? and(...conditions) : undefined;

      const [countResult] = await db
        .select({ total: count() })
        .from(sessions)
        .where(whereClause);

      const total = countResult?.total ?? 0;
      const offset = (input.page - 1) * input.limit;

      const rows = await db
        .select()
        .from(sessions)
        .where(whereClause)
        .orderBy(desc(sessions.startedAt))
        .limit(input.limit)
        .offset(offset);

      const mapped = rows.map((row) => ({
        id: row.id,
        project: row.project,
        command: row.command,
        status: row.status,
        trigger_type: row.triggerType,
        message_count: row.messageCount,
        tool_count: row.toolCount,
        started_at: row.startedAt.toISOString(),
        stopped_at: row.stoppedAt ? row.stoppedAt.toISOString() : null,
        duration_display: formatDuration(row.startedAt, row.stoppedAt),
      }));

      return {
        sessions: mapped,
        total,
        page: input.page,
        limit: input.limit,
      };
    }),

  /**
   * Get a single session by ID.
   */
  getById: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .query(async ({ input }) => {
      const [row] = await db
        .select()
        .from(sessions)
        .where(eq(sessions.id, input.id))
        .limit(1);

      if (!row) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Session not found",
        });
      }

      return {
        id: row.id,
        service: deriveService(row.command),
        status: mapStatus(row.status),
        messages: row.messageCount,
        tools_executed: row.toolCount,
        started_at: row.startedAt.toISOString(),
        ended_at: row.stoppedAt ? row.stoppedAt.toISOString() : null,
        project: row.project,
        trigger_type: row.triggerType,
        message_count: row.messageCount,
        tool_count: row.toolCount,
      };
    }),

  /**
   * Get aggregated session analytics.
   */
  analytics: protectedProcedure.query(async () => {
    const now = new Date();
    const startOfToday = new Date(
      Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()),
    );
    const start7d = new Date(startOfToday);
    start7d.setUTCDate(start7d.getUTCDate() - 6);

    const [todayResult, totalResult, avgDurationResult, daily7dResult, projectResult] =
      await Promise.all([
        db
          .select({ total: count() })
          .from(sessions)
          .where(sql`${sessions.startedAt} >= ${startOfToday}`),
        db.select({ total: count() }).from(sessions),
        db
          .select({
            avg: sql<string>`AVG(EXTRACT(EPOCH FROM (${sessions.stoppedAt} - ${sessions.startedAt})) / 60)`,
          })
          .from(sessions)
          .where(sql`${sessions.stoppedAt} IS NOT NULL`),
        db
          .select({
            date: sql<string>`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`,
            total: count(),
          })
          .from(sessions)
          .where(sql`${sessions.startedAt} >= ${start7d}`)
          .groupBy(sql`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`)
          .orderBy(sql`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`),
        db
          .select({
            project: sessions.project,
            total: count(),
          })
          .from(sessions)
          .groupBy(sessions.project)
          .orderBy(sql`COUNT(*) DESC`)
          .limit(8),
      ]);

    const dailyMap = new Map<string, number>();
    for (const row of daily7dResult) {
      dailyMap.set(row.date, row.total);
    }

    const sessions7d: { date: string; count: number }[] = [];
    for (let i = 6; i >= 0; i--) {
      const d = new Date(startOfToday);
      d.setUTCDate(d.getUTCDate() - i);
      const dateStr = d.toISOString().slice(0, 10);
      sessions7d.push({ date: dateStr, count: dailyMap.get(dateStr) ?? 0 });
    }

    return {
      sessions_today: todayResult[0]?.total ?? 0,
      sessions_7d: sessions7d,
      avg_duration_mins: parseFloat(avgDurationResult[0]?.avg ?? "0") || 0,
      project_breakdown: projectResult.map((r) => ({
        project: r.project,
        count: r.total,
      })),
      total_sessions: totalResult[0]?.total ?? 0,
    };
  }),

  /**
   * Get events for a session.
   */
  getEvents: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .query(async ({ input }) => {
      const rows = await db
        .select()
        .from(sessionEvents)
        .where(eq(sessionEvents.sessionId, input.id))
        .orderBy(asc(sessionEvents.createdAt));

      const events = rows.map((row) => ({
        id: row.id,
        session_id: row.sessionId,
        event_type: row.eventType,
        direction: row.direction,
        content: row.content,
        metadata: row.metadata as Record<string, unknown> | null,
        created_at: row.createdAt.toISOString(),
      }));

      return { events };
    }),

  /**
   * Get CC-type sessions (identified by "claude" in command).
   */
  ccSessions: protectedProcedure.query(async () => {
    const rows = await db
      .select()
      .from(sessions)
      .where(like(sessions.command, "%claude%"))
      .orderBy(desc(sessions.startedAt));

    const mapped = rows.map((row) => ({
      id: row.id,
      project: row.project,
      state: row.status,
      machine_name: "homelab",
      started_at: row.startedAt.toISOString(),
      duration_display: formatDuration(row.startedAt, row.stoppedAt),
      restart_attempts: 0,
    }));

    return {
      sessions: mapped,
      configured: true,
    };
  }),
});
