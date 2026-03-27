import { type NextRequest, NextResponse } from "next/server";
import { and, count, desc, eq, gte, lte } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessions } from "@nova/db";
import type { SessionTimelineItem, SessionListResponse } from "@/types/api";

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

export async function GET(request: NextRequest) {
  try {
    const url = request.nextUrl;
    const page = Math.max(1, Number(url.searchParams.get("page") ?? "1"));
    const limit = Math.min(100, Math.max(1, Number(url.searchParams.get("limit") ?? "25")));
    const project = url.searchParams.get("project");
    const triggerType = url.searchParams.get("trigger_type");
    const dateFrom = url.searchParams.get("date_from");
    const dateTo = url.searchParams.get("date_to");

    // Build where conditions
    const conditions = [];
    if (project) {
      conditions.push(eq(sessions.project, project));
    }
    if (triggerType) {
      conditions.push(eq(sessions.triggerType, triggerType));
    }
    if (dateFrom) {
      conditions.push(gte(sessions.startedAt, new Date(dateFrom)));
    }
    if (dateTo) {
      // Include the entire end date by adding 1 day
      const endDate = new Date(dateTo);
      endDate.setDate(endDate.getDate() + 1);
      conditions.push(lte(sessions.startedAt, endDate));
    }

    const whereClause = conditions.length > 0 ? and(...conditions) : undefined;

    // Get total count
    const [countResult] = await db
      .select({ total: count() })
      .from(sessions)
      .where(whereClause);

    const total = countResult?.total ?? 0;

    // Get paginated rows
    const offset = (page - 1) * limit;
    const rows = await db
      .select()
      .from(sessions)
      .where(whereClause)
      .orderBy(desc(sessions.startedAt))
      .limit(limit)
      .offset(offset);

    const mapped: SessionTimelineItem[] = rows.map((row) => ({
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

    const response: SessionListResponse = {
      sessions: mapped,
      total,
      page,
      limit,
    };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
