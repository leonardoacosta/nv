import { type NextRequest, NextResponse } from "next/server";
import { desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";
import type { ObligationActivity } from "@/types/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const limit = parseInt(searchParams.get("limit") ?? "20", 10);

    const rows = await db
      .select()
      .from(obligations)
      .orderBy(desc(obligations.updatedAt))
      .limit(limit);

    const events: ObligationActivity[] = rows.map((row) => ({
      id: row.id,
      event_type: "status_change",
      obligation_id: row.id,
      description: `${row.detectedAction} — ${row.status}`,
      timestamp: row.updatedAt.toISOString(),
    }));

    return NextResponse.json({ events });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
