import { NextResponse } from "next/server";
import type { ApiProject, ProjectsGetResponse } from "@/types/api";

const DEFAULT_PROJECTS: ApiProject[] = [
  { code: "nv", path: "~/dev/nv" },
];

export async function GET() {
  try {
    const envProjects = process.env.NV_PROJECTS;
    let projects: ApiProject[];

    if (envProjects) {
      try {
        projects = JSON.parse(envProjects);
      } catch {
        projects = DEFAULT_PROJECTS;
      }
    } else {
      projects = DEFAULT_PROJECTS;
    }

    const response: ProjectsGetResponse = { projects };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
