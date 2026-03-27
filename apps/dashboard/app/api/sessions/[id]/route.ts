import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessions } from "@nova/db";
import type { SessionDetail } from "@/types/api";

/**
 * Derive a human-readable service label from the raw command string.
 * - Commands containing "claude" → "CLI"
 * - Commands containing "telegram" → "Telegram"
 * - Otherwise fall back to the raw command value.
 */
function deriveService(command: string): string {
  const lower = command.toLowerCase();
  if (lower.includes("claude")) return "CLI";
  if (lower.includes("telegram")) return "Telegram";
  return command;
}

/**
 * Map a DB status value to the SessionDetail status field.
 * "running" → "active"; all other values pass through unchanged.
 */
function mapStatus(status: string): string {
  return status === "running" ? "active" : status;
}

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    const rows = await db
      .select()
      .from(sessions)
      .where(eq(sessions.id, id))
      .limit(1);

    const row = rows[0];

    if (!row) {
      return NextResponse.json({ error: "Session not found" }, { status: 404 });
    }

    const detail: SessionDetail = {
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

    return NextResponse.json(detail);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
