import { type NextRequest, NextResponse } from "next/server";
import { desc, eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

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
      .select({
        id: messages.id,
        channel: messages.channel,
        sender: messages.sender,
        content: messages.content,
        metadata: messages.metadata,
        createdAt: messages.createdAt,
      })
      .from(messages)
      .where(where)
      .orderBy(desc(messages.createdAt))
      .limit(limit)
      .offset(offset);

    const mapped = rows.map((row) =>
      toSnakeCase(row as unknown as Record<string, unknown>),
    );

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
