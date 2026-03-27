import { NextResponse } from "next/server";
import { desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages } from "@nova/db";

export async function GET() {
  try {
    const rows = await db
      .select()
      .from(messages)
      .orderBy(desc(messages.createdAt))
      .limit(50);

    // Map to StoredMessage shape matching existing /api/messages contract
    const mapped = rows.map((row, idx) => ({
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

    return NextResponse.json({
      messages: mapped,
      limit: 50,
      offset: 0,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
