import { type NextRequest, NextResponse } from "next/server";
import { and, count, eq, inArray, max } from "drizzle-orm";
import { db } from "@/lib/db";
import {
  projects,
  obligations,
  sessions,
  createProjectSchema,
} from "@nova/db";
import type {
  ProjectEntity,
  ProjectsListResponse,
  ProjectCategory,
  ProjectStatus,
} from "@/types/api";

interface EnvProject {
  code: string;
  path?: string;
}

const DEFAULT_PROJECTS: EnvProject[] = [{ code: "nv", path: "~/dev/nv" }];

const ACTIVE_OBLIGATION_STATUSES = ["open", "in_progress"];

/**
 * Query projects from DB, enrich with obligation/session counts.
 * If the table is empty, seed from NV_PROJECTS env var first.
 */
export async function GET(request: NextRequest) {
  try {
    const category = request.nextUrl.searchParams.get("category");

    // Check if projects table has rows
    let projectRows = await db.select().from(projects);

    // Seed from NV_PROJECTS if empty
    if (projectRows.length === 0) {
      const envProjects = process.env.NV_PROJECTS;
      let seedList: EnvProject[];

      if (envProjects) {
        try {
          seedList = JSON.parse(envProjects) as EnvProject[];
        } catch {
          seedList = DEFAULT_PROJECTS;
        }
      } else {
        seedList = DEFAULT_PROJECTS;
      }

      for (const p of seedList) {
        await db.insert(projects).values({
          code: p.code,
          name: p.code,
          category: "work",
          status: "active",
          path: p.path ?? null,
        }).onConflictDoNothing();
      }

      // Re-fetch after seeding
      projectRows = await db.select().from(projects);
    }

    // Apply category filter if provided
    if (category) {
      projectRows = projectRows.filter((p) => p.category === category);
    }

    if (projectRows.length === 0) {
      const response: ProjectsListResponse = { projects: [] };
      return NextResponse.json(response);
    }

    const codes = projectRows.map((p) => p.code);

    // Obligation counts per project_code
    const obligationTotals = await db
      .select({
        projectCode: obligations.projectCode,
        total: count(obligations.id),
      })
      .from(obligations)
      .where(inArray(obligations.projectCode, codes))
      .groupBy(obligations.projectCode);

    const obligationActive = await db
      .select({
        projectCode: obligations.projectCode,
        activeTotal: count(obligations.id),
      })
      .from(obligations)
      .where(
        and(
          inArray(obligations.projectCode, codes),
          inArray(obligations.status, ACTIVE_OBLIGATION_STATUSES),
        ),
      )
      .groupBy(obligations.projectCode);

    // Session counts + last startedAt per project
    const sessionStats = await db
      .select({
        project: sessions.project,
        sessionCount: count(sessions.id),
        lastStarted: max(sessions.startedAt),
      })
      .from(sessions)
      .where(inArray(sessions.project, codes))
      .groupBy(sessions.project);

    // Build lookup maps
    const totalByCode = new Map<string, number>();
    for (const row of obligationTotals) {
      if (row.projectCode) totalByCode.set(row.projectCode, row.total);
    }

    const activeByCode = new Map<string, number>();
    for (const row of obligationActive) {
      if (row.projectCode) activeByCode.set(row.projectCode, row.activeTotal);
    }

    const sessionByCode = new Map<string, { sessionCount: number; lastStarted: Date | null }>();
    for (const row of sessionStats) {
      sessionByCode.set(row.project, {
        sessionCount: row.sessionCount,
        lastStarted: row.lastStarted ?? null,
      });
    }

    // Assemble enriched projects
    const enriched: ProjectEntity[] = projectRows.map((p) => {
      const sessionInfo = sessionByCode.get(p.code);
      const lastActivity = sessionInfo?.lastStarted
        ? sessionInfo.lastStarted.toISOString()
        : null;

      return {
        id: p.id,
        code: p.code,
        name: p.name,
        category: p.category as ProjectCategory,
        status: p.status as ProjectStatus,
        description: p.description,
        content: p.content,
        path: p.path,
        obligation_count: totalByCode.get(p.code) ?? 0,
        active_obligation_count: activeByCode.get(p.code) ?? 0,
        session_count: sessionInfo?.sessionCount ?? 0,
        last_activity: lastActivity,
        created_at: p.createdAt.toISOString(),
        updated_at: p.updatedAt.toISOString(),
      };
    });

    const response: ProjectsListResponse = { projects: enriched };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

/**
 * Create a new project.
 */
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const parsed = createProjectSchema.safeParse(body);

    if (!parsed.success) {
      return NextResponse.json(
        { error: "Validation failed", details: parsed.error.flatten().fieldErrors },
        { status: 400 },
      );
    }

    // Check for duplicate code
    const existing = await db
      .select({ id: projects.id })
      .from(projects)
      .where(eq(projects.code, parsed.data.code))
      .limit(1);

    if (existing.length > 0) {
      return NextResponse.json(
        { error: `Project with code '${parsed.data.code}' already exists` },
        { status: 409 },
      );
    }

    const [created] = await db
      .insert(projects)
      .values({
        code: parsed.data.code,
        name: parsed.data.name,
        category: parsed.data.category ?? "work",
        status: parsed.data.status ?? "active",
        path: parsed.data.path ?? null,
      })
      .returning();

    if (!created) {
      return NextResponse.json({ error: "Failed to create project" }, { status: 500 });
    }

    const entity: ProjectEntity = {
      id: created.id,
      code: created.code,
      name: created.name,
      category: created.category as ProjectCategory,
      status: created.status as ProjectStatus,
      description: created.description,
      content: created.content,
      path: created.path,
      obligation_count: 0,
      active_obligation_count: 0,
      session_count: 0,
      last_activity: null,
      created_at: created.createdAt.toISOString(),
      updated_at: created.updatedAt.toISOString(),
    };

    return NextResponse.json(entity, { status: 201 });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
