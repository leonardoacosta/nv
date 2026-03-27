import { NextResponse } from "next/server";
import { count, sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { sessions } from "@nova/db";
import type { SessionAnalyticsResponse } from "@/types/api";

export async function GET() {
  try {
    const now = new Date();

    // Start of today in UTC (midnight).
    const startOfToday = new Date(
      Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()),
    );

    // Start of 7 days ago (inclusive of today = last 7 calendar days).
    const start7d = new Date(startOfToday);
    start7d.setUTCDate(start7d.getUTCDate() - 6);

    // Run all aggregate queries in parallel.
    const [
      todayResult,
      totalResult,
      avgDurationResult,
      daily7dResult,
      projectResult,
    ] = await Promise.all([
      // sessions_today
      db
        .select({ total: count() })
        .from(sessions)
        .where(sql`${sessions.startedAt} >= ${startOfToday}`),

      // total_sessions
      db.select({ total: count() }).from(sessions),

      // avg_duration_mins — completed sessions only
      db
        .select({
          avg: sql<string>`AVG(EXTRACT(EPOCH FROM (${sessions.stoppedAt} - ${sessions.startedAt})) / 60)`,
        })
        .from(sessions)
        .where(sql`${sessions.stoppedAt} IS NOT NULL`),

      // sessions_7d — daily counts for last 7 days
      db
        .select({
          date: sql<string>`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`,
          total: count(),
        })
        .from(sessions)
        .where(sql`${sessions.startedAt} >= ${start7d}`)
        .groupBy(sql`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`)
        .orderBy(sql`DATE(${sessions.startedAt} AT TIME ZONE 'UTC')`),

      // project_breakdown — top 8 projects by session count
      db
        .select({
          project: sessions.project,
          total: count(),
        })
        .from(sessions)
        .groupBy(sessions.project)
        .orderBy(sql`COUNT(*) DESC`)
        .limit(8),
    ]);

    // Build the full 7-day date series, filling in zeros for days with no sessions.
    const dailyMap = new Map<string, number>();
    for (const row of daily7dResult) {
      dailyMap.set(row.date, row.total);
    }

    const sessions7d: { date: string; count: number }[] = [];
    for (let i = 6; i >= 0; i--) {
      const d = new Date(startOfToday);
      d.setUTCDate(d.getUTCDate() - i);
      const dateStr = d.toISOString().slice(0, 10);
      sessions7d.push({ date: dateStr, count: dailyMap.get(dateStr) ?? 0 });
    }

    const response: SessionAnalyticsResponse = {
      sessions_today: todayResult[0]?.total ?? 0,
      sessions_7d: sessions7d,
      avg_duration_mins: parseFloat(avgDurationResult[0]?.avg ?? "0") || 0,
      project_breakdown: projectResult.map((r) => ({
        project: r.project,
        count: r.total,
      })),
      total_sessions: totalResult[0]?.total ?? 0,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
