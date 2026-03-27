import { type NextRequest, NextResponse } from "next/server";
import { asc, count, desc, eq, ne, sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages } from "@nova/db";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const channel = searchParams.get("channel");
    const direction = searchParams.get("direction"); // "inbound" | "outbound"
    const sort = searchParams.get("sort") ?? "desc"; // "asc" | "desc"
    const type = searchParams.get("type"); // "conversation" | "tool-call" | "system"
    const limitParam = searchParams.get("limit");
    const offsetParam = searchParams.get("offset");
    const limit = limitParam ? parseInt(limitParam, 10) : 50;
    const offset = offsetParam ? parseInt(offsetParam, 10) : 0;

    // Build WHERE conditions.
    const conditions = [];

    if (channel) {
      conditions.push(eq(messages.channel, channel));
    }

    if (direction === "outbound") {
      conditions.push(eq(messages.sender, "nova"));
    } else if (direction === "inbound") {
      conditions.push(ne(messages.sender, "nova"));
    }

    if (type) {
      // Filter on metadata->>'type' JSONB field.
      conditions.push(
        sql`${messages.metadata}->>'type' = ${type}`,
      );
    }

    const where =
      conditions.length === 0
        ? undefined
        : conditions.length === 1
          ? conditions[0]
          : sql`${conditions.reduce((acc, cond) => sql`${acc} AND ${cond}`)}`;

    const orderBy =
      sort === "asc" ? asc(messages.createdAt) : desc(messages.createdAt);

    // Run data query and count query in parallel.
    const [rows, countResult] = await Promise.all([
      db
        .select()
        .from(messages)
        .where(where)
        .orderBy(orderBy)
        .limit(limit)
        .offset(offset),
      db
        .select({ total: count() })
        .from(messages)
        .where(where),
    ]);

    const total = countResult[0]?.total ?? 0;

    // Map to StoredMessage shape expected by the frontend.
    const mapped = rows.map((row, idx) => {
      const metadata = row.metadata as Record<string, unknown> | null;
      const messageType =
        typeof metadata?.type === "string"
          ? (metadata.type as "conversation" | "tool-call" | "system")
          : "conversation";

      return {
        id: idx + offset,
        timestamp: row.createdAt.toISOString(),
        direction: row.sender === "nova" ? "outbound" : "inbound",
        channel: row.channel ?? "unknown",
        sender: row.sender ?? "unknown",
        content: row.content,
        response_time_ms: null,
        tokens_in: null,
        tokens_out: null,
        type: messageType,
      };
    });

    return NextResponse.json({
      messages: mapped,
      total,
      limit,
      offset,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
