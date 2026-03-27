import { NextResponse } from "next/server";
import { and, eq, gte, sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";

export async function GET() {
  try {
    // Count obligations by status and owner groupings
    const statusCounts = await db
      .select({
        status: obligations.status,
        owner: obligations.owner,
        count: sql<number>`count(*)::int`,
      })
      .from(obligations)
      .groupBy(obligations.status, obligations.owner);

    let openNova = 0;
    let openLeo = 0;
    let inProgress = 0;
    let proposedDone = 0;

    for (const row of statusCounts) {
      if (row.status === "open" && row.owner === "nova") openNova += row.count;
      if (row.status === "open" && row.owner === "leo") openLeo += row.count;
      if (row.status === "in_progress") inProgress += row.count;
      if (row.status === "proposed_done") proposedDone += row.count;
    }

    // Count obligations completed today
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);

    const [doneTodayResult] = await db
      .select({ count: sql<number>`count(*)::int` })
      .from(obligations)
      .where(
        and(
          eq(obligations.status, "done"),
          gte(obligations.updatedAt, todayStart),
        ),
      );

    return NextResponse.json({
      open_nova: openNova,
      open_leo: openLeo,
      in_progress: inProgress,
      proposed_done: proposedDone,
      done_today: doneTodayResult?.count ?? 0,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
