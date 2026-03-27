import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { memory } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(request: NextRequest) {
  try {
    const topic = request.nextUrl.searchParams.get("topic");

    if (topic) {
      const row = await db
        .select()
        .from(memory)
        .where(eq(memory.topic, topic))
        .limit(1);

      if (row.length === 0) {
        return NextResponse.json({ topic, content: "" });
      }

      return NextResponse.json({
        topic,
        content: row[0].content,
      });
    }

    // No topic — return list of all topics
    const rows = await db
      .select({
        id: memory.id,
        topic: memory.topic,
        updatedAt: memory.updatedAt,
      })
      .from(memory);

    const mapped = rows.map((row) =>
      toSnakeCase(row as unknown as Record<string, unknown>),
    );

    return NextResponse.json({ topics: mapped });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();
    const { topic: topicValue, content } = body as {
      topic: string;
      content: string;
    };

    if (!topicValue || content == null) {
      return NextResponse.json(
        { error: "topic and content are required" },
        { status: 400 },
      );
    }

    await db
      .insert(memory)
      .values({
        topic: topicValue,
        content,
        updatedAt: new Date(),
      })
      .onConflictDoUpdate({
        target: memory.topic,
        set: {
          content,
          updatedAt: new Date(),
        },
      });

    return NextResponse.json({
      topic: topicValue,
      written: content.length,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
