import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const body = await request.json();

    // Map incoming snake_case body fields to camelCase for Drizzle
    const updates: Record<string, unknown> = {};
    if (body.status !== undefined) updates.status = body.status;
    if (body.owner !== undefined) updates.owner = body.owner;
    if (body.priority !== undefined) updates.priority = body.priority;
    if (body.detected_action !== undefined)
      updates.detectedAction = body.detected_action;
    if (body.project_code !== undefined)
      updates.projectCode = body.project_code;
    if (body.deadline !== undefined) updates.deadline = body.deadline;
    updates.updatedAt = new Date();

    const [updated] = await db
      .update(obligations)
      .set(updates)
      .where(eq(obligations.id, id))
      .returning();

    if (!updated) {
      return NextResponse.json(
        { error: "Obligation not found" },
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
