import { type NextRequest, NextResponse } from "next/server";
import { eq, like, and } from "drizzle-orm";
import { db } from "@/lib/db";
import { contacts } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const relationship = searchParams.get("relationship");
    const q = searchParams.get("q");

    const conditions = [];
    if (relationship)
      conditions.push(eq(contacts.relationshipType, relationship));
    if (q) conditions.push(like(contacts.name, `%${q}%`));

    const where =
      conditions.length === 0
        ? undefined
        : conditions.length === 1
          ? conditions[0]
          : and(...conditions);

    const rows = await db.select().from(contacts).where(where);

    return NextResponse.json(
      rows.map((r) => toSnakeCase(r as unknown as Record<string, unknown>)),
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();

    const [created] = await db
      .insert(contacts)
      .values({
        name: body.name,
        channelIds: body.channel_ids ?? {},
        relationshipType: body.relationship_type ?? null,
        notes: body.notes ?? null,
      })
      .returning();

    return NextResponse.json(
      toSnakeCase(created as unknown as Record<string, unknown>),
      { status: 201 },
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
