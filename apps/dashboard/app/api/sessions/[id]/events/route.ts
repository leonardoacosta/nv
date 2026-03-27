import { type NextRequest, NextResponse } from "next/server";
import { asc, eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessionEvents } from "@nova/db";
import type { SessionEventItem, SessionEventsResponse } from "@/types/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    const rows = await db
      .select()
      .from(sessionEvents)
      .where(eq(sessionEvents.sessionId, id))
      .orderBy(asc(sessionEvents.createdAt));

    const events: SessionEventItem[] = rows.map((row) => ({
      id: row.id,
      session_id: row.sessionId,
      event_type: row.eventType,
      direction: row.direction,
      content: row.content,
      metadata: row.metadata as Record<string, unknown> | null,
      created_at: row.createdAt.toISOString(),
    }));

    const response: SessionEventsResponse = { events };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
