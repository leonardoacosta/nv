import type { ServiceConfig } from "../config.js";
import { adoRestWithRetry } from "./ado-rest.js";
import { sanitize } from "../utils.js";

// ── Work Items ──────────────────────────────────────────────────────────

/**
 * Query Azure DevOps work items with optional filters.
 * REST: POST https://dev.azure.com/{org}/{project}/_apis/wit/wiql
 *
 * Work items use WIQL (Work Item Query Language) via REST POST.
 * Note: `az boards query` also uses the azure-devops extension, so this
 * is equally broken via CLI. REST fixes it.
 */
export async function adoWorkItems(
  config: ServiceConfig,
  project?: string,
  state?: string,
  type?: string,
  limit: number = 20,
): Promise<string> {
  // Build WIQL
  const clauses: string[] = [];
  if (project) {
    clauses.push(`[System.TeamProject] = '${sanitize(project)}'`);
  }
  if (state) {
    clauses.push(`[System.State] = '${sanitize(state)}'`);
  }
  if (type) {
    clauses.push(`[System.WorkItemType] = '${sanitize(type)}'`);
  }

  const where = clauses.length > 0 ? ` WHERE ${clauses.join(" AND ")}` : "";
  const wiql = `SELECT [System.Id], [System.Title], [System.State], [System.WorkItemType], [System.AssignedTo] FROM WorkItems${where} ORDER BY [System.ChangedDate] DESC`;

  // WIQL endpoint -- if project is given, scope to it; otherwise org-wide
  const path = project
    ? `${sanitize(project)}/_apis/wit/wiql`
    : `_apis/wit/wiql`;

  const raw = await adoRestWithRetry(config.cloudpcHost, path, {
    method: "POST",
    body: { query: wiql },
    query: { $top: limit },
  });

  // WIQL returns only IDs; fetch details for each
  const wiqlResult = JSON.parse(raw);
  const ids: number[] = (wiqlResult.workItems ?? [])
    .slice(0, limit)
    .map((wi: { id: number }) => wi.id);

  if (ids.length === 0) {
    return JSON.stringify({ count: 0, value: [] }, null, 2);
  }

  // Batch fetch work item details (max 200 per call)
  const detailsRaw = await adoRestWithRetry(
    config.cloudpcHost,
    `_apis/wit/workitems`,
    {
      query: {
        ids: ids.join(","),
        fields:
          "System.Id,System.Title,System.State,System.WorkItemType,System.AssignedTo,System.TeamProject",
      },
    },
  );

  return detailsRaw;
}

// ── Repositories ────────────────────────────────────────────────────────

/**
 * List Azure DevOps repositories in a project.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/git/repositories
 */
export async function adoRepos(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  if (project) {
    return adoRestWithRetry(
      config.cloudpcHost,
      `${sanitize(project)}/_apis/git/repositories`,
    );
  }

  // Org-wide
  return adoRestWithRetry(
    config.cloudpcHost,
    `_apis/git/repositories`,
  );
}

// ── Pull Requests ───────────────────────────────────────────────────────

/**
 * List Azure DevOps pull requests with optional filters.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/git/pullrequests
 */
export async function adoPullRequests(
  config: ServiceConfig,
  project?: string,
  status?: string,
): Promise<string> {
  const query: Record<string, string | number | boolean | undefined> = {};

  // Map friendly status names to REST API values
  if (status) {
    const s = status.toLowerCase();
    if (s === "active") query["searchCriteria.status"] = "active";
    else if (s === "completed") query["searchCriteria.status"] = "completed";
    else if (s === "abandoned") query["searchCriteria.status"] = "abandoned";
    else query["searchCriteria.status"] = s;
  }

  const path = project
    ? `${sanitize(project)}/_apis/git/pullrequests`
    : `_apis/git/pullrequests`;

  return adoRestWithRetry(config.cloudpcHost, path, { query });
}

// ── Build Logs ──────────────────────────────────────────────────────────

/**
 * Get details and logs for a specific Azure DevOps build run.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/build/builds/{buildId}
 *       GET https://dev.azure.com/{org}/{project}/_apis/build/builds/{buildId}/logs
 *
 * Since a build always belongs to a project, we need the project. If not
 * provided, we'll try to find the build across projects.
 */
export async function adoBuildLogs(
  config: ServiceConfig,
  buildId: number,
  project?: string,
): Promise<string> {
  if (project) {
    // Get build details
    const buildRaw = await adoRestWithRetry(
      config.cloudpcHost,
      `${sanitize(project)}/_apis/build/builds/${buildId}`,
    );

    // Also fetch the log list
    let logs: string | undefined;
    try {
      logs = await adoRestWithRetry(
        config.cloudpcHost,
        `${sanitize(project)}/_apis/build/builds/${buildId}/logs`,
      );
    } catch {
      // Logs may not be available for queued builds
    }

    const build = JSON.parse(buildRaw);
    const logData = logs ? JSON.parse(logs) : { count: 0, value: [] };

    return JSON.stringify({ build, logs: logData }, null, 2);
  }

  // No project -- search across projects
  const projRaw = await adoRestWithRetry(
    config.cloudpcHost,
    "_apis/projects",
  );
  const projData = JSON.parse(projRaw);

  for (const p of (projData.value ?? []).slice(0, 10)) {
    try {
      const buildRaw = await adoRestWithRetry(
        config.cloudpcHost,
        `${encodeURIComponent(p.name)}/_apis/build/builds/${buildId}`,
      );
      return buildRaw;
    } catch {
      // Not in this project
    }
  }

  return JSON.stringify({ error: `Build ${buildId} not found in any accessible project` });
}

// ── Git: Commits ────────────────────────────────────────────────────────

/**
 * Get recent commits from an Azure DevOps Git repository.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repo}/commits
 */
export async function adoCommits(
  config: ServiceConfig,
  repoName: string,
  project?: string,
  limit: number = 20,
): Promise<string> {
  if (!project) {
    // Try to find the repo across projects
    const projRaw = await adoRestWithRetry(
      config.cloudpcHost,
      "_apis/projects",
    );
    const projData = JSON.parse(projRaw);

    for (const p of (projData.value ?? []).slice(0, 10)) {
      try {
        return await adoRestWithRetry(
          config.cloudpcHost,
          `${encodeURIComponent(p.name)}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/commits`,
          { query: { "searchCriteria.$top": limit } },
        );
      } catch {
        // Repo not in this project
      }
    }
    return JSON.stringify({ error: `Repository '${repoName}' not found` });
  }

  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/commits`,
    { query: { "searchCriteria.$top": limit } },
  );
}

// ── Pipeline Definition ─────────────────────────────────────────────────

/**
 * Get pipeline definition details.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/build/definitions/{id}
 *
 * Uses the Build Definitions API which includes triggers, variables, etc.
 */
export async function adoPipelineDefinition(
  config: ServiceConfig,
  pipelineId: number,
  project?: string,
): Promise<string> {
  if (project) {
    return adoRestWithRetry(
      config.cloudpcHost,
      `${sanitize(project)}/_apis/build/definitions/${pipelineId}`,
    );
  }

  // Search across projects
  const projRaw = await adoRestWithRetry(
    config.cloudpcHost,
    "_apis/projects",
  );
  const projData = JSON.parse(projRaw);

  for (const p of (projData.value ?? []).slice(0, 10)) {
    try {
      return await adoRestWithRetry(
        config.cloudpcHost,
        `${encodeURIComponent(p.name)}/_apis/build/definitions/${pipelineId}`,
      );
    } catch {
      // Not in this project
    }
  }

  return JSON.stringify({ error: `Pipeline ${pipelineId} not found` });
}

// ── Pipeline Update ─────────────────────────────────────────────────────

/**
 * Update a pipeline's default branch or settings.
 * REST: PATCH https://dev.azure.com/{org}/{project}/_apis/build/definitions/{id}
 *
 * Must first GET the current definition, modify it, then PUT back.
 */
export async function adoPipelineUpdate(
  config: ServiceConfig,
  pipelineId: number,
  project: string,
  branch?: string,
): Promise<string> {
  // Get current definition
  const currentRaw = await adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/build/definitions/${pipelineId}`,
  );
  const current = JSON.parse(currentRaw);

  // Modify
  if (branch) {
    current.repository = current.repository ?? {};
    current.repository.defaultBranch = branch;
  }

  // PUT updated definition
  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/build/definitions/${pipelineId}`,
    {
      method: "PUT",
      body: current,
    },
  );
}

// ── Repo Update ─────────────────────────────────────────────────────────

/**
 * Set default branch for a repository.
 * REST: PATCH https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repoId}
 */
export async function adoRepoUpdate(
  config: ServiceConfig,
  repoName: string,
  project: string,
  defaultBranch: string,
): Promise<string> {
  // First, resolve repo name to ID
  const reposRaw = await adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}`,
  );
  const repo = JSON.parse(reposRaw);

  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/git/repositories/${repo.id}`,
    {
      method: "PATCH",
      body: { defaultBranch },
    },
  );
}

// ── Pipeline Run ────────────────────────────────────────────────────────

/**
 * Trigger a pipeline run.
 * REST: POST https://dev.azure.com/{org}/{project}/_apis/pipelines/{id}/runs
 */
export async function adoPipelineRun(
  config: ServiceConfig,
  pipelineId: number,
  project: string,
  branch?: string,
): Promise<string> {
  const body: Record<string, unknown> = {};
  if (branch) {
    body.resources = {
      repositories: {
        self: { refName: branch },
      },
    };
  }

  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/pipelines/${pipelineId}/runs`,
    {
      method: "POST",
      body,
    },
  );
}

// ── Pipeline Variables ──────────────────────────────────────────────────

/**
 * List variables configured on a pipeline.
 * REST: GET the build definition and extract variables.
 */
export async function adoPipelineVariables(
  config: ServiceConfig,
  pipelineId: number,
  project?: string,
): Promise<string> {
  const defRaw = await adoPipelineDefinition(config, pipelineId, project);
  const def = JSON.parse(defRaw);

  if (def.error) return defRaw;

  const variables = def.variables ?? {};
  return JSON.stringify(variables, null, 2);
}

// ── Branches ────────────────────────────────────────────────────────────

/**
 * List branches in a repository.
 * REST: GET https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repo}/refs
 */
export async function adoBranches(
  config: ServiceConfig,
  repoName: string,
  project?: string,
): Promise<string> {
  if (!project) {
    const projRaw = await adoRestWithRetry(
      config.cloudpcHost,
      "_apis/projects",
    );
    const projData = JSON.parse(projRaw);

    for (const p of (projData.value ?? []).slice(0, 10)) {
      try {
        return await adoRestWithRetry(
          config.cloudpcHost,
          `${encodeURIComponent(p.name)}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/refs`,
          { query: { filter: "heads" } },
        );
      } catch {
        // Repo not in this project
      }
    }
    return JSON.stringify({ error: `Repository '${repoName}' not found` });
  }

  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/refs`,
    { query: { filter: "heads" } },
  );
}

// ── Repo Delete ─────────────────────────────────────────────────────────

/**
 * Delete a repository.
 * REST: DELETE https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repoId}
 */
export async function adoRepoDelete(
  config: ServiceConfig,
  repoId: string,
  project: string,
): Promise<string> {
  return adoRestWithRetry(
    config.cloudpcHost,
    `${sanitize(project)}/_apis/git/repositories/${sanitize(repoId)}`,
    { method: "DELETE" },
  );
}
