import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { schedules } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();

    if (typeof body.enabled !== "boolean") {
      return NextResponse.json(
        { error: "Invalid body. 'enabled' must be a boolean." },
        { status: 400 },
      );
    }

    const [updated] = await db
      .update(schedules)
      .set({ enabled: body.enabled })
      .where(eq(schedules.id, id))
      .returning();

    if (!updated) {
      return NextResponse.json(
        { error: "Schedule not found" },
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
