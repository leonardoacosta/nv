import { type NextRequest, NextResponse } from "next/server";
import { desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { briefings } from "@nova/db";
import type {
  BriefingEntry,
  BriefingAction,
  BriefingHistoryGetResponse,
} from "@/types/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const limitParam = searchParams.get("limit");
    const limit = limitParam ? parseInt(limitParam, 10) : 20;

    const rows = await db
      .select()
      .from(briefings)
      .orderBy(desc(briefings.generatedAt))
      .limit(limit);

    const entries: BriefingEntry[] = rows.map((row) => ({
      id: row.id,
      generated_at: row.generatedAt.toISOString(),
      content: row.content,
      suggested_actions: (row.suggestedActions as BriefingAction[]) ?? [],
      sources_status:
        (row.sourcesStatus as Record<string, string>) ?? {},
    }));

    const response: BriefingHistoryGetResponse = { entries };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
