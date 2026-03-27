import { NextResponse } from "next/server";
import { and, asc, desc, eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { reminders, schedules, sessions, briefings } from "@nova/db";
import type {
  AutomationReminder,
  AutomationSchedule,
  AutomationSession,
  AutomationBriefing,
  AutomationWatcher,
  AutomationsGetResponse,
} from "@/types/api";

// ── Cron helpers ─────────────────────────────────────────────────────────────

/**
 * Compute a rough next-run time from a cron expression and the last run time.
 * Handles common patterns; returns null for complex expressions.
 */
function computeNextRun(cronExpr: string, lastRunAt: Date | null): string | null {
  const now = new Date();
  const parts = cronExpr.trim().split(/\s+/);
  if (parts.length !== 5) return null;

  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  // "0 7 * * *" — daily at HH:MM
  if (dayOfMonth === "*" && month === "*" && dayOfWeek === "*" && !minute!.includes("/") && !hour!.includes("/")) {
    const m = parseInt(minute!, 10);
    const h = parseInt(hour!, 10);
    if (isNaN(m) || isNaN(h)) return null;
    const next = new Date(now);
    next.setHours(h, m, 0, 0);
    if (next <= now) next.setDate(next.getDate() + 1);
    return next.toISOString();
  }

  // "*/N * * * *" — every N minutes
  if (minute!.startsWith("*/") && hour === "*" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    const interval = parseInt(minute!.slice(2), 10);
    if (isNaN(interval) || interval <= 0) return null;
    const base = lastRunAt ?? now;
    const next = new Date(base.getTime() + interval * 60_000);
    return next > now ? next.toISOString() : new Date(now.getTime() + interval * 60_000).toISOString();
  }

  return null;
}

// ── GET handler ──────────────────────────────────────────────────────────────

export async function GET() {
  try {
    const now = new Date();

    // Query reminders: non-cancelled, non-delivered, ordered by due_at asc
    const reminderRows = await db
      .select()
      .from(reminders)
      .where(
        and(
          eq(reminders.cancelled, false),
        ),
      )
      .orderBy(asc(reminders.dueAt));

    // Filter out delivered reminders in JS (deliveredAt is nullable, no eq(null) in drizzle)
    const activeReminders = reminderRows.filter((r) => r.deliveredAt === null);

    const mappedReminders: AutomationReminder[] = activeReminders.map((r) => ({
      id: r.id,
      message: r.message,
      due_at: r.dueAt.toISOString(),
      channel: r.channel,
      created_at: r.createdAt.toISOString(),
      status: r.dueAt < now ? "overdue" : "pending",
    }));

    // Query schedules: all, ordered by name asc
    const scheduleRows = await db
      .select()
      .from(schedules)
      .orderBy(asc(schedules.name));

    const mappedSchedules: AutomationSchedule[] = scheduleRows.map((s) => ({
      id: s.id,
      name: s.name,
      cron_expr: s.cronExpr,
      action: s.action,
      channel: s.channel,
      enabled: s.enabled,
      last_run_at: s.lastRunAt?.toISOString() ?? null,
      next_run: s.enabled ? computeNextRun(s.cronExpr, s.lastRunAt) : null,
    }));

    // Query sessions: running, ordered by started_at desc
    const sessionRows = await db
      .select()
      .from(sessions)
      .where(eq(sessions.status, "running"))
      .orderBy(desc(sessions.startedAt));

    const mappedSessions: AutomationSession[] = sessionRows.map((s) => ({
      id: s.id,
      project: s.project,
      command: s.command,
      status: s.status,
      started_at: s.startedAt.toISOString(),
    }));

    // Query briefings: latest by generated_at
    const [latestBriefing] = await db
      .select()
      .from(briefings)
      .orderBy(desc(briefings.generatedAt))
      .limit(1);

    // Strip markdown formatting and truncate to 200 chars for the preview
    const contentPreview: string | null = latestBriefing
      ? latestBriefing.content
          // Remove headings (# ## ### etc.)
          .replace(/^#{1,6}\s+/gm, "")
          // Remove bold/italic markers
          .replace(/\*{1,3}([^*]+)\*{1,3}/g, "$1")
          .replace(/_{1,3}([^_]+)_{1,3}/g, "$1")
          // Remove inline code
          .replace(/`([^`]+)`/g, "$1")
          // Remove code blocks
          .replace(/```[\s\S]*?```/g, "")
          // Remove blockquotes
          .replace(/^>\s+/gm, "")
          // Remove horizontal rules
          .replace(/^[-*_]{3,}\s*$/gm, "")
          // Remove list markers
          .replace(/^[\s]*[-*+]\s+/gm, "")
          .replace(/^[\s]*\d+\.\s+/gm, "")
          // Collapse whitespace and newlines
          .replace(/\n{2,}/g, " ")
          .replace(/\n/g, " ")
          .replace(/\s{2,}/g, " ")
          .trim()
          .slice(0, 200)
      : null;

    const briefingData: AutomationBriefing = {
      last_generated_at: latestBriefing?.generatedAt.toISOString() ?? null,
      content_preview: contentPreview,
      next_generation: (() => {
        // Next 7:00 AM after now
        const next = new Date(now);
        next.setHours(7, 0, 0, 0);
        if (next <= now) next.setDate(next.getDate() + 1);
        // If already generated today, push to tomorrow
        if (latestBriefing) {
          const lastGen = latestBriefing.generatedAt;
          if (
            lastGen.getFullYear() === now.getFullYear() &&
            lastGen.getMonth() === now.getMonth() &&
            lastGen.getDate() === now.getDate()
          ) {
            const tomorrow = new Date(now);
            tomorrow.setDate(tomorrow.getDate() + 1);
            tomorrow.setHours(7, 0, 0, 0);
            return tomorrow.toISOString();
          }
        }
        return next.toISOString();
      })(),
    };

    // Watcher config: read from env or default
    const watcherData: AutomationWatcher = {
      enabled: process.env.WATCHER_ENABLED !== "false",
      interval_minutes: parseInt(process.env.WATCHER_INTERVAL_MINUTES ?? "30", 10),
      quiet_start: process.env.WATCHER_QUIET_START ?? "22:00",
      quiet_end: process.env.WATCHER_QUIET_END ?? "07:00",
      last_run_at: null,
    };

    const response: AutomationsGetResponse = {
      reminders: mappedReminders,
      schedules: mappedSchedules,
      watcher: watcherData,
      briefing: briefingData,
      active_sessions: mappedSessions,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
