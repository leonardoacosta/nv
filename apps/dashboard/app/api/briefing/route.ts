import { NextResponse } from "next/server";
import { desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { briefings } from "@nova/db";
import type {
  BriefingEntry,
  BriefingAction,
  BriefingGetResponse,
} from "@/types/api";

export async function GET() {
  try {
    const [latest] = await db
      .select()
      .from(briefings)
      .orderBy(desc(briefings.generatedAt))
      .limit(1);

    if (!latest) {
      const response: BriefingGetResponse = { entry: null };
      return NextResponse.json(response);
    }

    const entry: BriefingEntry = {
      id: latest.id,
      generated_at: latest.generatedAt.toISOString(),
      content: latest.content,
      suggested_actions: (latest.suggestedActions as BriefingAction[]) ?? [],
      sources_status:
        (latest.sourcesStatus as Record<string, string>) ?? {},
    };

    const response: BriefingGetResponse = { entry };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
