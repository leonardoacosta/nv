import { NextResponse } from "next/server";
import { desc, like } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessions } from "@nova/db";
import type { CcSessionSummary, CcSessionsGetResponse } from "@/types/api";

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

export async function GET() {
  try {
    // CC sessions are identified by the "claude" command pattern
    const rows = await db
      .select()
      .from(sessions)
      .where(like(sessions.command, "%claude%"))
      .orderBy(desc(sessions.startedAt));

    const mapped: CcSessionSummary[] = rows.map((row) => ({
      id: row.id,
      project: row.project,
      state: row.status,
      machine_name: "homelab",
      started_at: row.startedAt.toISOString(),
      duration_display: formatDuration(row.startedAt, row.stoppedAt),
      restart_attempts: 0,
    }));

    const response: CcSessionsGetResponse = {
      sessions: mapped,
      configured: true,
    };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
