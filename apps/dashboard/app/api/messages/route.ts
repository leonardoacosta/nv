import { type NextRequest, NextResponse } from "next/server";
import { desc, eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages } from "@nova/db";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const channel = searchParams.get("channel");
    const limitParam = searchParams.get("limit");
    const offsetParam = searchParams.get("offset");
    const limit = limitParam ? parseInt(limitParam, 10) : 50;
    const offset = offsetParam ? parseInt(offsetParam, 10) : 0;

    const where = channel ? eq(messages.channel, channel) : undefined;

    const rows = await db
      .select()
      .from(messages)
      .where(where)
      .orderBy(desc(messages.createdAt))
      .limit(limit)
      .offset(offset);

    // Map to StoredMessage shape expected by the frontend
    const mapped = rows.map((row, idx) => ({
      id: idx + offset,
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
      limit,
      offset,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
