import { NextResponse } from "next/server";
import { desc, gte } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages, obligations, diary, sessions } from "@nova/db";
import type { ActivityFeedEvent, ActivityFeedGetResponse } from "@/types/api";

function getObligationSeverity(status: string): "error" | "warning" | "info" {
  const lower = status.toLowerCase();
  if (lower.includes("failed") || lower.includes("error")) return "error";
  if (lower === "open" || lower.includes("detected")) return "warning";
  return "info";
}

export async function GET() {
  try {
    const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);

    const [messageRows, obligationRows, diaryRows, sessionRows] = await Promise.all([
      db
        .select()
        .from(messages)
        .where(gte(messages.createdAt, twentyFourHoursAgo))
        .orderBy(desc(messages.createdAt))
        .limit(50),
      db
        .select()
        .from(obligations)
        .where(gte(obligations.createdAt, twentyFourHoursAgo))
        .orderBy(desc(obligations.createdAt))
        .limit(50),
      db
        .select()
        .from(diary)
        .where(gte(diary.createdAt, twentyFourHoursAgo))
        .orderBy(desc(diary.createdAt))
        .limit(50),
      db
        .select()
        .from(sessions)
        .where(gte(sessions.startedAt, twentyFourHoursAgo))
        .orderBy(desc(sessions.startedAt))
        .limit(50),
    ]);

    const events: ActivityFeedEvent[] = [];

    for (const row of messageRows) {
      const direction = row.sender === "nova" ? "outbound" : "inbound";
      const preview = row.content.length > 80 ? `${row.content.slice(0, 80)}...` : row.content;
      events.push({
        id: `msg-${row.id}`,
        type: "message",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "MessageSquare",
        summary: `${direction === "inbound" ? "In" : "Out"} [${row.channel}] ${row.sender ?? "unknown"}: ${preview}`,
        severity: "info",
      });
    }

    for (const row of obligationRows) {
      events.push({
        id: `obl-${row.id}`,
        type: "obligation",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "CheckSquare",
        summary: `${row.detectedAction} — ${row.status}`,
        severity: getObligationSeverity(row.status),
      });
    }

    for (const row of diaryRows) {
      events.push({
        id: `diary-${row.id}`,
        type: "diary",
        timestamp: row.createdAt.toISOString(),
        icon_hint: "BookOpen",
        summary: `${row.slug} [${row.channel}]`,
        severity: "info",
      });
    }

    for (const row of sessionRows) {
      const isCompleted = row.status === "completed" || row.status === "stopped";
      const summary = isCompleted
        ? `Session completed: ${row.project} (${row.command})`
        : `Session started: ${row.project} (${row.command})`;
      events.push({
        id: `session-${row.id}`,
        type: "session",
        timestamp: row.startedAt.toISOString(),
        icon_hint: "Activity",
        summary,
        severity: "info",
      });
    }

    // Sort by timestamp descending, take top 50
    events.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
    const top50 = events.slice(0, 50);

    const response: ActivityFeedGetResponse = { events: top50 };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
