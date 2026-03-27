import { type NextRequest, NextResponse } from "next/server";
import { sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages } from "@nova/db";
import type { RelationshipsResponse } from "@/types/api";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const minCountParam = searchParams.get("min_count");
    const minCount = minCountParam ? parseInt(minCountParam, 10) : 2;

    // Find pairs of non-"nova" senders who messaged the same channel on the
    // same day. The count of distinct shared days is the co-occurrence score.
    // Each (person_a, person_b, channel) triple appears at most once.
    const rows = await db.execute<{
      person_a: string;
      person_b: string;
      shared_channel: string;
      co_occurrence_count: number;
    }>(sql`
      WITH sender_days AS (
        SELECT DISTINCT sender, channel, DATE(created_at) AS day
        FROM ${messages}
        WHERE sender IS NOT NULL
          AND LOWER(sender) != 'nova'
      )
      SELECT
        a.sender  AS person_a,
        b.sender  AS person_b,
        a.channel AS shared_channel,
        COUNT(*)::int AS co_occurrence_count
      FROM sender_days a
      JOIN sender_days b
        ON a.channel = b.channel
        AND a.day = b.day
        AND a.sender < b.sender
      GROUP BY a.sender, b.sender, a.channel
      HAVING COUNT(*) >= ${minCount}
      ORDER BY co_occurrence_count DESC
    `);

    const response: RelationshipsResponse = {
      relationships: (rows as unknown as { person_a: string; person_b: string; shared_channel: string; co_occurrence_count: number }[]).map((r) => ({
        person_a: r.person_a,
        person_b: r.person_b,
        shared_channel: r.shared_channel,
        co_occurrence_count: r.co_occurrence_count,
      })),
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
