import { type NextRequest, NextResponse } from "next/server";
import { and, gte, lt, desc, sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { diary } from "@nova/db";
import type { DiaryEntryItem, DiaryGetResponse } from "@/types/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const dateParam = searchParams.get("date");
    const limitParam = searchParams.get("limit");
    const limit = limitParam ? parseInt(limitParam, 10) : 50;

    const conditions = [];
    let dateLabel = new Date().toISOString().slice(0, 10);

    if (dateParam) {
      dateLabel = dateParam;
      const dayStart = new Date(`${dateParam}T00:00:00.000Z`);
      const dayEnd = new Date(`${dateParam}T23:59:59.999Z`);
      conditions.push(gte(diary.createdAt, dayStart));
      conditions.push(lt(diary.createdAt, dayEnd));
    }

    const where =
      conditions.length === 0
        ? undefined
        : conditions.length === 1
          ? conditions[0]
          : and(...conditions);

    const rows = await db
      .select()
      .from(diary)
      .where(where)
      .orderBy(desc(diary.createdAt))
      .limit(limit);

    // Count total matching rows
    const [totalResult] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(diary)
      .where(where);

    // Count distinct channels
    const [channelResult] = await db
      .select({ count: sql<number>`count(distinct ${diary.channel})::int` })
      .from(diary)
      .where(where);

    // Get last interaction timestamp
    const [lastResult] = await db
      .select({ last: sql<string>`max(${diary.createdAt})` })
      .from(diary)
      .where(where);

    const entries: DiaryEntryItem[] = rows.map((row) => ({
      time: row.createdAt.toISOString(),
      trigger_type: row.triggerType,
      trigger_source: row.triggerSource,
      channel_source: row.channel,
      slug: row.slug,
      tools_called: (row.toolsUsed as string[] | null) ?? [],
      result_summary: row.content,
      response_latency_ms: row.responseLatencyMs ?? 0,
      tokens_in: row.tokensIn ?? 0,
      tokens_out: row.tokensOut ?? 0,
    }));

    const response: DiaryGetResponse = {
      date: dateLabel,
      entries,
      total: totalResult?.count ?? entries.length,
      distinct_channels: channelResult?.count ?? 0,
      last_interaction_at: lastResult?.last ?? null,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
