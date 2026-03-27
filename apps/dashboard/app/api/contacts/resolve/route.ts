import { type NextRequest, NextResponse } from "next/server";
import { db } from "@/lib/db";
import { contacts } from "@nova/db";

/**
 * POST /api/contacts/resolve
 *
 * Accepts `{ senders: string[] }` — an array of raw sender identifiers
 * (e.g. "telegram:7380462766") — and returns a `Record<string, string>`
 * mapping each sender to a resolved contact display name.
 *
 * Resolution strategy: fetch all contacts, then for each contact check
 * whether any value in the `channelIds` JSONB matches one of the
 * requested sender identifiers. The contact table is small (tens of rows)
 * so a full in-memory scan is acceptable.
 */
export async function POST(request: NextRequest) {
  try {
    const body = await request.json() as { senders?: unknown };

    if (!Array.isArray(body.senders)) {
      return NextResponse.json(
        { error: "senders must be an array of strings" },
        { status: 400 },
      );
    }

    const senders = body.senders as string[];

    if (senders.length === 0) {
      return NextResponse.json({} as Record<string, string>);
    }

    const rows = await db.select().from(contacts);

    const result: Record<string, string> = {};

    for (const row of rows) {
      const channelIds = row.channelIds as Record<string, string | undefined> | null;
      if (!channelIds) continue;

      // Check each channel entry value against the requested senders.
      // channelIds shape: { telegram: "7380462766", discord: "...", ... }
      // sender format: "telegram:7380462766"
      for (const [platform, platformId] of Object.entries(channelIds)) {
        if (!platformId) continue;
        const composed = `${platform}:${platformId}`;
        if (senders.includes(composed)) {
          result[composed] = row.name;
        }
        // Also match on bare platform ID without prefix.
        if (senders.includes(platformId)) {
          result[platformId] = row.name;
        }
      }
    }

    return NextResponse.json(result);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
