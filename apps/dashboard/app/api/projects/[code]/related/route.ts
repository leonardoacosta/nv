import { type NextRequest, NextResponse } from "next/server";
import { desc, eq, ilike, like } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations, sessions, memory, messages } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ code: string }> },
) {
  try {
    const { code } = await params;

    // [4.1] Query obligations for this project, ordered by updatedAt desc
    const obligationRows = await db
      .select()
      .from(obligations)
      .where(eq(obligations.projectCode, code))
      .orderBy(desc(obligations.updatedAt));

    const mappedObligations = obligationRows.map((row) => ({
      ...toSnakeCase(row as unknown as Record<string, unknown>),
      notes: [],
      attempt_count: 0,
    }));

    // Obligation summary
    const obligationSummary = {
      total: obligationRows.length,
      open: obligationRows.filter((o) => o.status === "open").length,
      in_progress: obligationRows.filter((o) => o.status === "in_progress").length,
      done: obligationRows.filter((o) => o.status === "done").length,
    };

    // [4.2] Query sessions for this project, ordered by startedAt desc
    const sessionRows = await db
      .select()
      .from(sessions)
      .where(eq(sessions.project, code))
      .orderBy(desc(sessions.startedAt));

    const sessionCount = sessionRows.length;

    const mappedSessions = sessionRows.map((row) => ({
      id: row.id,
      project: row.project,
      status: row.status,
      agent_name: row.command,
      started_at: row.startedAt?.toISOString(),
      duration_display: row.stoppedAt
        ? formatDuration(row.startedAt, row.stoppedAt)
        : "running",
      branch: undefined,
      spec: undefined,
      progress: undefined,
    }));

    // [4.3] Query memory topics starting with `projects-`, filter by project code in app
    const allProjectTopics = await db
      .select({ topic: memory.topic, content: memory.content })
      .from(memory)
      .where(like(memory.topic, "projects-%"));

    const matchingTopics = allProjectTopics
      .filter((t) => t.content.toLowerCase().includes(code.toLowerCase()))
      .map((t) => ({
        topic: t.topic,
        preview: t.content.slice(0, 500),
      }));

    // [4.4] Recent messages mentioning the project code (case-insensitive), limit 20
    const recentMessageRows = await db
      .select()
      .from(messages)
      .where(ilike(messages.content, `%${code}%`))
      .orderBy(desc(messages.createdAt))
      .limit(20);

    const mappedRecentMessages = recentMessageRows.map((row, idx) => ({
      id: idx,
      timestamp: row.createdAt.toISOString(),
      direction: row.sender === "nova" ? "outbound" : "inbound",
      channel: row.channel ?? "unknown",
      sender: row.sender ?? "unknown",
      content: row.content,
      response_time_ms: null,
      tokens_in: null,
      tokens_out: null,
    }));

    // [4.5] Assemble ProjectRelatedResponse
    const response = {
      project: { code, path: "" },
      obligations: mappedObligations,
      obligation_summary: obligationSummary,
      sessions: mappedSessions,
      session_count: sessionCount,
      memory_topics: matchingTopics,
      recent_messages: mappedRecentMessages,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

function formatDuration(start: Date, stop: Date): string {
  const ms = stop.getTime() - start.getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  const hours = Math.floor(mins / 60);
  return `${hours}h ${mins % 60}m`;
}
