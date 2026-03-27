import { NextResponse } from "next/server";
import { sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { messages, contacts } from "@nova/db";
import type { DiscoveredContactsResponse } from "@/types/api";

export async function GET() {
  try {
    // Aggregate unique senders from messages, excluding "nova" (the bot itself).
    // LEFT JOIN contacts for enrichment (name override, relationship, notes).
    const rows = await db.execute<{
      sender: string;
      message_count: number;
      channels: string;
      first_seen: string;
      last_seen: string;
      contact_id: string | null;
      relationship_type: string | null;
      notes: string | null;
      channel_ids: Record<string, string> | null;
    }>(sql`
      SELECT
        m.sender,
        COUNT(*)::int                          AS message_count,
        STRING_AGG(DISTINCT m.channel, ',')    AS channels,
        MIN(m.created_at)::text                AS first_seen,
        MAX(m.created_at)::text                AS last_seen,
        c.id                                   AS contact_id,
        c.relationship_type                    AS relationship_type,
        c.notes                                AS notes,
        c.channel_ids                          AS channel_ids
      FROM ${messages} m
      LEFT JOIN ${contacts} c ON LOWER(c.name) = LOWER(m.sender)
      WHERE m.sender IS NOT NULL
        AND LOWER(m.sender) != 'nova'
      GROUP BY m.sender, c.id, c.relationship_type, c.notes, c.channel_ids
      ORDER BY MAX(m.created_at) DESC
    `);

    const totalMessages = await db
      .execute<{ count: number }>(
        sql`SELECT COUNT(*)::int AS count FROM ${messages}`,
      )
      .then((r) => (r as unknown as { count: number }[])[0]?.count ?? 0);

    const mapped = (rows as unknown as { sender: string; message_count: number; channels: string; first_seen: string; last_seen: string; contact_id: string | null; relationship_type: string | null; notes: string | null }[]).map((r) => ({
      name: r.sender,
      channels: r.channels ? r.channels.split(",") : [],
      message_count: r.message_count,
      first_seen: r.first_seen,
      last_seen: r.last_seen,
      contact_id: r.contact_id,
      relationship_type: r.relationship_type,
      notes: r.notes,
      channel_ids: null,
    }));

    const response: DiscoveredContactsResponse = {
      contacts: mapped,
      total_senders: mapped.length,
      total_messages_scanned: totalMessages,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
