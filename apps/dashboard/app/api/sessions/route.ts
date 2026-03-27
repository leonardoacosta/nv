import { NextResponse } from "next/server";
import { desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessions } from "@nova/db";
import type { NexusSessionRaw, SessionsGetResponse } from "@/types/api";

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
    const rows = await db
      .select()
      .from(sessions)
      .orderBy(desc(sessions.startedAt));

    const mapped: NexusSessionRaw[] = rows.map((row) => ({
      id: row.id,
      project: row.project,
      status: row.status,
      agent_name: row.command,
      started_at: row.startedAt.toISOString(),
      duration_display: formatDuration(row.startedAt, row.stoppedAt),
    }));

    const response: SessionsGetResponse = { sessions: mapped };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
