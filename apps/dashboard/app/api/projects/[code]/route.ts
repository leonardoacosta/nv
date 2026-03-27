import { type NextRequest, NextResponse } from "next/server";
import { eq } from "drizzle-orm";
import { db } from "@/lib/db";
import { projects, updateProjectSchema } from "@nova/db";
import type { ProjectEntity, ProjectCategory, ProjectStatus } from "@/types/api";

/**
 * Update a project by its code.
 */
export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ code: string }> },
) {
  try {
    const { code } = await params;
    const body = await request.json();
    const parsed = updateProjectSchema.safeParse(body);

    if (!parsed.success) {
      return NextResponse.json(
        { error: "Validation failed", details: parsed.error.flatten().fieldErrors },
        { status: 400 },
      );
    }

    // Check existence
    const existing = await db
      .select()
      .from(projects)
      .where(eq(projects.code, code))
      .limit(1);

    if (existing.length === 0) {
      return NextResponse.json(
        { error: `Project with code '${code}' not found` },
        { status: 404 },
      );
    }

    const [updated] = await db
      .update(projects)
      .set({
        ...parsed.data,
        updatedAt: new Date(),
      })
      .where(eq(projects.code, code))
      .returning();

    if (!updated) {
      return NextResponse.json({ error: "Failed to update project" }, { status: 500 });
    }

    const entity: ProjectEntity = {
      id: updated.id,
      code: updated.code,
      name: updated.name,
      category: updated.category as ProjectCategory,
      status: updated.status as ProjectStatus,
      description: updated.description,
      content: updated.content,
      path: updated.path,
      obligation_count: 0,
      active_obligation_count: 0,
      session_count: 0,
      last_activity: null,
      created_at: updated.createdAt.toISOString(),
      updated_at: updated.updatedAt.toISOString(),
    };

    return NextResponse.json(entity);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
