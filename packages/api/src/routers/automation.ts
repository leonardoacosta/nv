import { and, asc, count, desc, eq, inArray, ne, sql } from "drizzle-orm";
import { z } from "zod";
import { TRPCError } from "@trpc/server";

import { db } from "@nova/db";
import {
  reminders,
  schedules,
  sessions,
  briefings,
  settings,
  obligations,
  memory,
  messages,
} from "@nova/db";

import { createTRPCRouter, protectedProcedure } from "../trpc.js";

// ── Cron helpers ─────────────────────────────────────────────────────

function computeNextRun(
  cronExpr: string,
  lastRunAt: Date | null,
): string | null {
  const now = new Date();
  const parts = cronExpr.trim().split(/\s+/);
  if (parts.length !== 5) return null;

  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  // "0 7 * * *" -- daily at HH:MM
  if (
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*" &&
    !minute!.includes("/") &&
    !hour!.includes("/")
  ) {
    const m = parseInt(minute!, 10);
    const h = parseInt(hour!, 10);
    if (isNaN(m) || isNaN(h)) return null;
    const next = new Date(now);
    next.setHours(h, m, 0, 0);
    if (next <= now) next.setDate(next.getDate() + 1);
    return next.toISOString();
  }

  // "*/N * * * *" -- every N minutes
  if (
    minute!.startsWith("*/") &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    const interval = parseInt(minute!.slice(2), 10);
    if (isNaN(interval) || interval <= 0) return null;
    const base = lastRunAt ?? now;
    const next = new Date(base.getTime() + interval * 60_000);
    return next > now
      ? next.toISOString()
      : new Date(now.getTime() + interval * 60_000).toISOString();
  }

  return null;
}

// HH:MM validation (24-hour format)
const HH_MM_RE = /^([01]\d|2[0-3]):([0-5]\d)$/;

// In-memory watcher config override (reverts on process restart)
const watcherOverrides: Partial<{
  enabled: boolean;
  interval_minutes: number;
  quiet_start: string;
  quiet_end: string;
}> = {};

function getWatcherState() {
  return {
    enabled:
      watcherOverrides.enabled ??
      process.env.WATCHER_ENABLED !== "false",
    interval_minutes:
      watcherOverrides.interval_minutes ??
      parseInt(process.env.WATCHER_INTERVAL_MINUTES ?? "30", 10),
    quiet_start:
      watcherOverrides.quiet_start ??
      (process.env.WATCHER_QUIET_START ?? "22:00"),
    quiet_end:
      watcherOverrides.quiet_end ??
      (process.env.WATCHER_QUIET_END ?? "07:00"),
    last_run_at: null as string | null,
  };
}

const ALLOWED_SETTING_KEYS = new Set([
  "watcher_prompt",
  "briefing_prompt",
  "briefing_hour",
]);

export const automationRouter = createTRPCRouter({
  /**
   * Get full automations overview (reminders, schedules, watcher, briefing, active sessions).
   */
  getAll: protectedProcedure.query(async () => {
    const now = new Date();

    // Active reminders
    const reminderRows = await db
      .select()
      .from(reminders)
      .where(and(eq(reminders.cancelled, false)))
      .orderBy(asc(reminders.dueAt));

    const activeReminders = reminderRows.filter(
      (r) => r.deliveredAt === null,
    );

    const mappedReminders = activeReminders.map((r) => ({
      id: r.id,
      message: r.message,
      due_at: r.dueAt.toISOString(),
      channel: r.channel,
      created_at: r.createdAt.toISOString(),
      status: (r.dueAt < now ? "overdue" : "pending") as
        | "overdue"
        | "pending",
    }));

    // Schedules
    const scheduleRows = await db
      .select()
      .from(schedules)
      .orderBy(asc(schedules.name));

    const mappedSchedules = scheduleRows.map((s) => ({
      id: s.id,
      name: s.name,
      cron_expr: s.cronExpr,
      action: s.action,
      channel: s.channel,
      enabled: s.enabled,
      last_run_at: s.lastRunAt?.toISOString() ?? null,
      next_run: s.enabled ? computeNextRun(s.cronExpr, s.lastRunAt) : null,
    }));

    // Active sessions
    const sessionRows = await db
      .select()
      .from(sessions)
      .where(eq(sessions.status, "running"))
      .orderBy(desc(sessions.startedAt));

    const mappedSessions = sessionRows.map((s) => ({
      id: s.id,
      project: s.project,
      command: s.command,
      status: s.status,
      started_at: s.startedAt.toISOString(),
    }));

    // Latest briefing
    const [latestBriefing] = await db
      .select()
      .from(briefings)
      .orderBy(desc(briefings.generatedAt))
      .limit(1);

    // Briefing hour from settings
    const [briefingHourSetting] = await db
      .select()
      .from(settings)
      .where(eq(settings.key, "briefing_hour"));

    const briefingHour = briefingHourSetting
      ? parseInt(briefingHourSetting.value, 10)
      : 7;
    const effectiveBriefingHour = isNaN(briefingHour) ? 7 : briefingHour;

    // Content preview
    const contentPreview: string | null = latestBriefing
      ? latestBriefing.content
          .replace(/^#{1,6}\s+/gm, "")
          .replace(/\*{1,3}([^*]+)\*{1,3}/g, "$1")
          .replace(/_{1,3}([^_]+)_{1,3}/g, "$1")
          .replace(/`([^`]+)`/g, "$1")
          .replace(/```[\s\S]*?```/g, "")
          .replace(/^>\s+/gm, "")
          .replace(/^[-*_]{3,}\s*$/gm, "")
          .replace(/^[\s]*[-*+]\s+/gm, "")
          .replace(/^[\s]*\d+\.\s+/gm, "")
          .replace(/\n{2,}/g, " ")
          .replace(/\n/g, " ")
          .replace(/\s{2,}/g, " ")
          .trim()
          .slice(0, 200)
      : null;

    const nextGeneration = (() => {
      const next = new Date(now);
      next.setHours(effectiveBriefingHour, 0, 0, 0);
      if (next <= now) next.setDate(next.getDate() + 1);
      if (latestBriefing) {
        const lastGen = latestBriefing.generatedAt;
        if (
          lastGen.getFullYear() === now.getFullYear() &&
          lastGen.getMonth() === now.getMonth() &&
          lastGen.getDate() === now.getDate()
        ) {
          const tomorrow = new Date(now);
          tomorrow.setDate(tomorrow.getDate() + 1);
          tomorrow.setHours(effectiveBriefingHour, 0, 0, 0);
          return tomorrow.toISOString();
        }
      }
      return next.toISOString();
    })();

    const briefingData = {
      last_generated_at: latestBriefing?.generatedAt.toISOString() ?? null,
      content_preview: contentPreview,
      briefing_hour: effectiveBriefingHour,
      next_generation: nextGeneration,
    };

    const watcherData = getWatcherState();

    return {
      reminders: mappedReminders,
      schedules: mappedSchedules,
      watcher: watcherData,
      briefing: briefingData,
      active_sessions: mappedSessions,
    };
  }),

  /**
   * Create a new reminder.
   */
  listReminders: protectedProcedure.query(async () => {
    const now = new Date();
    const reminderRows = await db
      .select()
      .from(reminders)
      .where(and(eq(reminders.cancelled, false)))
      .orderBy(asc(reminders.dueAt));

    const activeReminders = reminderRows.filter(
      (r) => r.deliveredAt === null,
    );

    return activeReminders.map((r) => ({
      id: r.id,
      message: r.message,
      due_at: r.dueAt.toISOString(),
      channel: r.channel,
      created_at: r.createdAt.toISOString(),
      status: (r.dueAt < now ? "overdue" : "pending") as
        | "overdue"
        | "pending",
    }));
  }),

  /**
   * Cancel a reminder by ID.
   */
  updateReminder: protectedProcedure
    .input(
      z.object({
        id: z.string().uuid(),
        action: z.literal("cancel"),
      }),
    )
    .mutation(async ({ input }) => {
      const [updated] = await db
        .update(reminders)
        .set({ cancelled: true })
        .where(eq(reminders.id, input.id))
        .returning();

      if (!updated) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Reminder not found",
        });
      }

      return {
        id: updated.id,
        message: updated.message,
        due_at: updated.dueAt.toISOString(),
        channel: updated.channel,
        created_at: updated.createdAt.toISOString(),
        cancelled: updated.cancelled,
      };
    }),

  /**
   * List all schedules.
   */
  listSchedules: protectedProcedure.query(async () => {
    const scheduleRows = await db
      .select()
      .from(schedules)
      .orderBy(asc(schedules.name));

    return scheduleRows.map((s) => ({
      id: s.id,
      name: s.name,
      cron_expr: s.cronExpr,
      action: s.action,
      channel: s.channel,
      enabled: s.enabled,
      last_run_at: s.lastRunAt?.toISOString() ?? null,
      next_run: s.enabled ? computeNextRun(s.cronExpr, s.lastRunAt) : null,
    }));
  }),

  /**
   * Toggle a schedule enabled/disabled.
   */
  updateSchedule: protectedProcedure
    .input(
      z.object({
        id: z.string().uuid(),
        enabled: z.boolean(),
      }),
    )
    .mutation(async ({ input }) => {
      const [updated] = await db
        .update(schedules)
        .set({ enabled: input.enabled })
        .where(eq(schedules.id, input.id))
        .returning();

      if (!updated) {
        throw new TRPCError({
          code: "NOT_FOUND",
          message: "Schedule not found",
        });
      }

      return {
        id: updated.id,
        name: updated.name,
        cron_expr: updated.cronExpr,
        action: updated.action,
        channel: updated.channel,
        enabled: updated.enabled,
        last_run_at: updated.lastRunAt?.toISOString() ?? null,
      };
    }),

  /**
   * Get all settings.
   */
  getSettings: protectedProcedure.query(async () => {
    const rows = await db.select().from(settings);
    const settingsMap: Record<string, string> = {};
    for (const row of rows) {
      settingsMap[row.key] = row.value;
    }
    return { settings: settingsMap };
  }),

  /**
   * Update a setting (upsert by key).
   */
  updateSettings: protectedProcedure
    .input(
      z.object({
        key: z.string().min(1),
        value: z.string().min(1),
      }),
    )
    .mutation(async ({ input }) => {
      if (!ALLOWED_SETTING_KEYS.has(input.key)) {
        throw new TRPCError({
          code: "BAD_REQUEST",
          message: `Invalid key '${input.key}'. Allowed keys: ${[...ALLOWED_SETTING_KEYS].join(", ")}`,
        });
      }

      const [updated] = await db
        .insert(settings)
        .values({ key: input.key, value: input.value })
        .onConflictDoUpdate({
          target: settings.key,
          set: {
            value: input.value,
            updatedAt: sql`now()`,
          },
        })
        .returning();

      return updated;
    }),

  /**
   * Get watcher config and update it.
   */
  getWatcher: protectedProcedure.query(() => {
    return getWatcherState();
  }),

  /**
   * Update watcher configuration (in-memory, reverts on restart).
   */
  updateWatcher: protectedProcedure
    .input(
      z.object({
        enabled: z.boolean().optional(),
        interval_minutes: z.number().int().min(5).max(120).optional(),
        quiet_start: z.string().regex(HH_MM_RE).optional(),
        quiet_end: z.string().regex(HH_MM_RE).optional(),
      }),
    )
    .mutation(({ input }) => {
      if (input.enabled !== undefined) watcherOverrides.enabled = input.enabled;
      if (input.interval_minutes !== undefined)
        watcherOverrides.interval_minutes = input.interval_minutes;
      if (input.quiet_start !== undefined)
        watcherOverrides.quiet_start = input.quiet_start;
      if (input.quiet_end !== undefined)
        watcherOverrides.quiet_end = input.quiet_end;

      return getWatcherState();
    }),

  /**
   * Assemble a preview of the prompt context that would be sent to Nova for a
   * given automation type (watcher | briefing).
   *
   * Queries obligations, memory, and messages with a 5-second per-source
   * timeout via Promise.allSettled. Each section reports its own status
   * (ok / unavailable / empty) so the UI can surface partial failures.
   */
  previewContext: protectedProcedure
    .input(z.object({ type: z.enum(["watcher", "briefing"]) }))
    .query(async () => {
      const ACTIVE_OBLIGATION_STATUSES = ["open", "in_progress", "pending"] as const;
      const TIMEOUT_MS = 5_000;

      function withTimeout<T>(promise: Promise<T>): Promise<T> {
        return Promise.race([
          promise,
          new Promise<T>((_, reject) =>
            setTimeout(() => reject(new Error("timeout")), TIMEOUT_MS),
          ),
        ]);
      }

      const [obligationsResult, memoryResult, messagesResult] =
        await Promise.allSettled([
          withTimeout(
            db
              .select()
              .from(obligations)
              .where(inArray(obligations.status, [...ACTIVE_OBLIGATION_STATUSES]))
              .orderBy(desc(obligations.updatedAt))
              .limit(20),
          ),
          withTimeout(
            db
              .select()
              .from(memory)
              .orderBy(desc(memory.updatedAt))
              .limit(10),
          ),
          withTimeout(
            db
              .select()
              .from(messages)
              .orderBy(desc(messages.createdAt))
              .limit(50),
          ),
        ]);

      // ── Obligations ──────────────────────────────────────────────────────
      const obligationItems =
        obligationsResult.status === "fulfilled" ? obligationsResult.value : [];
      const obligationStatus =
        obligationsResult.status === "rejected"
          ? "unavailable"
          : obligationItems.length === 0
            ? "empty"
            : "ok";

      const countByStatus: Record<string, number> = {};
      for (const ob of obligationItems) {
        countByStatus[ob.status] = (countByStatus[ob.status] ?? 0) + 1;
      }

      const mappedObligations = obligationItems.map((ob) => ({
        id: ob.id,
        detectedAction: ob.detectedAction,
        status: ob.status,
        priority: ob.priority,
        sourceChannel: ob.sourceChannel,
        deadline: ob.deadline?.toISOString() ?? null,
        createdAt: ob.createdAt.toISOString(),
      }));

      // ── Memory ───────────────────────────────────────────────────────────
      const memoryItems =
        memoryResult.status === "fulfilled" ? memoryResult.value : [];
      const memoryStatus =
        memoryResult.status === "rejected"
          ? "unavailable"
          : memoryItems.length === 0
            ? "empty"
            : "ok";

      const mappedMemory = memoryItems.map((m) => ({
        topic: m.topic,
        contentPreview: m.content.slice(0, 200),
      }));

      // ── Messages ─────────────────────────────────────────────────────────
      const messageRows =
        messagesResult.status === "fulfilled" ? messagesResult.value : [];
      const messageStatus =
        messagesResult.status === "rejected"
          ? "unavailable"
          : messageRows.length === 0
            ? "empty"
            : "ok";

      // Group by channel
      const channelMap = new Map<
        string,
        { count: number; latest: string | null }
      >();
      for (const msg of messageRows) {
        const ch = msg.channel ?? "unknown";
        const existing = channelMap.get(ch);
        if (!existing) {
          channelMap.set(ch, {
            count: 1,
            latest: msg.content.slice(0, 120),
          });
        } else {
          existing.count += 1;
        }
      }

      const byChannel = Array.from(channelMap.entries()).map(
        ([channel, { count: msgCount, latest }]) => ({
          channel,
          count: msgCount,
          latestPreview: latest,
        }),
      );

      // Known channel names (for the pills UI)
      const KNOWN_CHANNELS = [
        "telegram",
        "discord",
        "teams",
        "email",
        "dashboard",
      ] as const;
      const channelInfos: Array<{
        name: string;
        messageCount: number;
        active: boolean;
      }> = KNOWN_CHANNELS.map((name) => {
        const entry = channelMap.get(name);
        return {
          name,
          messageCount: entry?.count ?? 0,
          active: (entry?.count ?? 0) > 0,
        };
      });

      // Also include any non-standard channels present in the data
      for (const [name, { count: msgCount }] of channelMap.entries()) {
        if (!KNOWN_CHANNELS.includes(name as (typeof KNOWN_CHANNELS)[number])) {
          channelInfos.push({ name, messageCount: msgCount, active: true });
        }
      }

      // ── Stats ────────────────────────────────────────────────────────────
      const stats = {
        totalObligations: obligationItems.length,
        activeReminders: 0, // not queried in this lightweight endpoint
        memoryTopics: memoryItems.length,
      };

      return {
        obligations: {
          status: obligationStatus as "ok" | "unavailable" | "empty",
          items: mappedObligations,
          countByStatus,
        },
        memory: {
          status: memoryStatus as "ok" | "unavailable" | "empty",
          items: mappedMemory,
        },
        messages: {
          status: messageStatus as "ok" | "unavailable" | "empty",
          byChannel,
        },
        channels: channelInfos,
        stats,
        assembledAt: new Date().toISOString(),
      };
    }),
});
