import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function POST(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    const [updated] = await db
      .update(obligations)
      .set({
        status: "in_progress",
        lastAttemptAt: new Date(),
        updatedAt: new Date(),
      })
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
