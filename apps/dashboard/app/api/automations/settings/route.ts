import { type NextRequest, NextResponse } from "next/server";
import { eq, sql } from "drizzle-orm";
import { db } from "@/lib/db";
import { settings } from "@nova/db";
import type { AutomationSettingsResponse, PutSettingRequest } from "@/types/api";

const ALLOWED_KEYS = new Set(["watcher_prompt", "briefing_prompt", "briefing_hour"]);

export async function GET() {
  try {
    const rows = await db.select().from(settings);

    const settingsMap: Record<string, string> = {};
    for (const row of rows) {
      settingsMap[row.key] = row.value;
    }

    const response: AutomationSettingsResponse = { settings: settingsMap };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function PUT(request: NextRequest) {
  try {
    const body = (await request.json()) as PutSettingRequest;

    if (!body.key || !body.value) {
      return NextResponse.json(
        { error: "Both 'key' and 'value' are required" },
        { status: 400 },
      );
    }

    if (!ALLOWED_KEYS.has(body.key)) {
      return NextResponse.json(
        {
          error: `Invalid key '${body.key}'. Allowed keys: ${[...ALLOWED_KEYS].join(", ")}`,
        },
        { status: 400 },
      );
    }

    const [updated] = await db
      .insert(settings)
      .values({ key: body.key, value: body.value })
      .onConflictDoUpdate({
        target: settings.key,
        set: {
          value: body.value,
          updatedAt: sql`now()`,
        },
      })
      .returning();

    return NextResponse.json(updated);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
