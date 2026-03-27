import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { reminders } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();

    if (body.action !== "cancel") {
      return NextResponse.json(
        { error: "Invalid action. Only 'cancel' is supported." },
        { status: 400 },
      );
    }

    const [updated] = await db
      .update(reminders)
      .set({ cancelled: true })
      .where(eq(reminders.id, id))
      .returning();

    if (!updated) {
      return NextResponse.json(
        { error: "Reminder not found" },
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
