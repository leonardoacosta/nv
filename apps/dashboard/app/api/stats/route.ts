import { NextResponse } from "next/server";
import { sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages, obligations, contacts, memory, diary } from "@nova/db";
import type { StatsGetResponse } from "@/types/api";

export async function GET() {
  try {
    const [msgCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(messages);
    const [oblCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(obligations);
    const [contactCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(contacts);
    const [memCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(memory);
    const [diaryCount] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(diary);

    const response: StatsGetResponse = {
      tool_usage: {
        total_invocations: 0,
        invocations_today: 0,
        per_tool: [],
      },
      counts: {
        messages: msgCount?.count ?? 0,
        obligations: oblCount?.count ?? 0,
        contacts: contactCount?.count ?? 0,
        memory: memCount?.count ?? 0,
        diary: diaryCount?.count ?? 0,
      },
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
