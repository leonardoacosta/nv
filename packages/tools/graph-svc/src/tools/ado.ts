import type { ServiceConfig } from "../config.js";
import { adoRestWithRetry } from "./ado-rest.js";
import { sanitize } from "../utils.js";

/**
 * List Azure DevOps projects.
 * REST: GET https://dev.azure.com/{org}/_apis/projects
 */
export async function adoProjects(config: ServiceConfig): Promise<string> {
  return adoRestWithRetry(config.cloudpcHost, "_apis/projects");
}

/**
 * List Azure DevOps pipelines, optionally filtered by project.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/pipelines
 *
 * Note: The Pipelines API requires a project. When no project is given,
 * we first list projects and then query each one (up to 10 for safety).
 */
export async function adoPipelines(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  if (project) {
    return adoRestWithRetry(
      config.cloudpcHost,
      `${sanitize(project)}/_apis/pipelines`,
    );
  }

  // No project specified -- list all pipelines across projects (up to 10 projects).
  const projRaw = await adoRestWithRetry(
    config.cloudpcHost,
    "_apis/projects",
  );
  const projData = JSON.parse(projRaw);
  const projects: { name: string }[] = projData.value ?? [];

  const allPipelines: unknown[] = [];
  for (const p of projects.slice(0, 10)) {
    try {
      const raw = await adoRestWithRetry(
        config.cloudpcHost,
        `${encodeURIComponent(p.name)}/_apis/pipelines`,
      );
      const data = JSON.parse(raw);
      if (data.value) {
        for (const pipeline of data.value) {
          allPipelines.push({ ...pipeline, project: p.name });
        }
      }
    } catch {
      // Skip projects where we lack permission
    }
  }

  return JSON.stringify({ count: allPipelines.length, value: allPipelines }, null, 2);
}

/**
 * Get recent Azure DevOps builds, optionally filtered by project, pipeline, and limited.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/build/builds
 */
export async function adoBuilds(
  config: ServiceConfig,
  project?: string,
  pipeline?: string,
  limit: number = 10,
): Promise<string> {
  if (project) {
    const query: Record<string, string | number | boolean | undefined> = {
      $top: limit,
    };

    // If filtering by pipeline name, first resolve the pipeline ID
    if (pipeline) {
      const pipRaw = await adoRestWithRetry(
        config.cloudpcHost,
        `${sanitize(project)}/_apis/pipelines`,
      );
      const pipData = JSON.parse(pipRaw);
      const match = (pipData.value ?? []).find(
        (p: { name: string }) =>
          p.name.toLowerCase() === pipeline.toLowerCase(),
      );
      if (match) {
        query.definitions = match.id;
      }
    }

    return adoRestWithRetry(
      config.cloudpcHost,
      `${sanitize(project)}/_apis/build/builds`,
      { query },
    );
  }

  // No project -- aggregate across projects (up to 10)
  const projRaw = await adoRestWithRetry(
    config.cloudpcHost,
    "_apis/projects",
  );
  const projData = JSON.parse(projRaw);
  const projects: { name: string }[] = projData.value ?? [];

  const allBuilds: unknown[] = [];
  for (const p of projects.slice(0, 10)) {
    try {
      const raw = await adoRestWithRetry(
        config.cloudpcHost,
        `${encodeURIComponent(p.name)}/_apis/build/builds`,
        { query: { $top: limit } },
      );
      const data = JSON.parse(raw);
      if (data.value) {
        for (const build of data.value) {
          allBuilds.push({ ...build, _project: p.name });
        }
      }
    } catch {
      // Skip inaccessible projects
    }
  }

  // Sort by start time descending, take top N
  allBuilds.sort((a: any, b: any) => {
    const ta = new Date(a.startTime ?? a.queueTime ?? 0).getTime();
    const tb = new Date(b.startTime ?? b.queueTime ?? 0).getTime();
    return tb - ta;
  });

  return JSON.stringify(
    { count: Math.min(allBuilds.length, limit), value: allBuilds.slice(0, limit) },
    null,
    2,
  );
}
