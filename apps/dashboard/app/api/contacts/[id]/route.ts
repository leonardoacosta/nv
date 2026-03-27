import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { contacts } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    const [contact] = await db
      .select()
      .from(contacts)
      .where(eq(contacts.id, id));

    if (!contact) {
      return NextResponse.json(
        { error: "Contact not found" },
        { status: 404 },
      );
    }

    return NextResponse.json(
      toSnakeCase(contact as unknown as Record<string, unknown>),
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();

    const [updated] = await db
      .update(contacts)
      .set({
        name: body.name,
        channelIds: body.channel_ids,
        relationshipType: body.relationship_type,
        notes: body.notes,
      })
      .where(eq(contacts.id, id))
      .returning();

    if (!updated) {
      return NextResponse.json(
        { error: "Contact not found" },
        { status: 404 },
      );
    }

    return NextResponse.json(
      toSnakeCase(updated as unknown as Record<string, unknown>),
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();

    const updates: Record<string, unknown> = {};
    if (body.name !== undefined) updates.name = body.name;
    if (body.channel_ids !== undefined) updates.channelIds = body.channel_ids;
    if (body.relationship_type !== undefined)
      updates.relationshipType = body.relationship_type;
    if (body.notes !== undefined) updates.notes = body.notes;

    const [updated] = await db
      .update(contacts)
      .set(updates)
      .where(eq(contacts.id, id))
      .returning();

    if (!updated) {
      return NextResponse.json(
        { error: "Contact not found" },
        { status: 404 },
      );
    }

    return NextResponse.json(
      toSnakeCase(updated as unknown as Record<string, unknown>),
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    const [deleted] = await db
      .delete(contacts)
      .where(eq(contacts.id, id))
      .returning();

    if (!deleted) {
      return NextResponse.json(
        { error: "Contact not found" },
        { status: 404 },
      );
    }

    return NextResponse.json(
      toSnakeCase(deleted as unknown as Record<string, unknown>),
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
