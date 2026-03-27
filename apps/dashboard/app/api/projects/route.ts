import { NextResponse } from "next/server";
import { db } from "@/lib/db";
import { enrichProjects } from "@/lib/entity-resolution";
import type { ApiProject } from "@/lib/entity-resolution";

const DEFAULT_PROJECTS: ApiProject[] = [
  { code: "nv", path: "~/dev/nv" },
];

export async function GET() {
  try {
    const envProjects = process.env.NV_PROJECTS;
    let baseProjects: ApiProject[];

    if (envProjects) {
      try {
        baseProjects = JSON.parse(envProjects) as ApiProject[];
      } catch {
        baseProjects = DEFAULT_PROJECTS;
      }
    } else {
      baseProjects = DEFAULT_PROJECTS;
    }

    // [6.1] Enrich the base project list with obligation/session counts + memory context
    const enriched = await enrichProjects(baseProjects, db);

    // Map to snake_case response shape for the frontend
    const projects = enriched.map((p) => ({
      code: p.code,
      path: p.path,
      description: p.description,
      memory_context: p.memoryContext,
      obligation_count: p.obligationCount,
      active_obligation_count: p.activeObligationCount,
      session_count: p.sessionCount,
      last_activity: p.lastActivity,
    }));

    return NextResponse.json({ projects });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
