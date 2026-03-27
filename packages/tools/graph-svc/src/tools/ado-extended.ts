import type { ServiceConfig } from "../config.js";
import { sshAdoCommand } from "../ssh.js";
import { socksGet, socksPost, socksPut, socksDelete, isSocksAvailable } from "../socks-client.js";
import { getAdoToken, clearTokenCache, ADO_ORG } from "./ado-rest.js";
import { sanitize } from "../utils.js";

/** Azure DevOps base URL. */
const ADO_BASE = `https://dev.azure.com/${ADO_ORG}`;
const API_VERSION = "7.0";
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

// ── Helpers ────────────────────────────────────────────────────────────

function buildUrl(path: string, query?: Record<string, string | number>): string {
  const url = new URL(`${ADO_BASE}/${path}`);
  url.searchParams.set("api-version", API_VERSION);
  if (query) {
    for (const [k, v] of Object.entries(query)) {
      if (v !== undefined) url.searchParams.set(k, String(v));
    }
  }
  return url.toString();
}

async function adoSocksGet(config: ServiceConfig, path: string, query?: Record<string, string | number>): Promise<string> {
  const url = buildUrl(path, query);
  const token = await getAdoToken(config.cloudpcHost);
  try {
    return await socksGet(url, token);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearTokenCache();
      const fresh = await getAdoToken(config.cloudpcHost);
      return await socksGet(url, fresh);
    }
    throw err;
  }
}

async function adoSocksPost(config: ServiceConfig, path: string, body: unknown, query?: Record<string, string | number>): Promise<string> {
  const url = buildUrl(path, query);
  const token = await getAdoToken(config.cloudpcHost);
  try {
    return await socksPost(url, token, body);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearTokenCache();
      const fresh = await getAdoToken(config.cloudpcHost);
      return await socksPost(url, fresh, body);
    }
    throw err;
  }
}

async function adoSocksPut(config: ServiceConfig, path: string, body: unknown, query?: Record<string, string | number>): Promise<string> {
  const url = buildUrl(path, query);
  const token = await getAdoToken(config.cloudpcHost);
  try {
    return await socksPut(url, token, body);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearTokenCache();
      const fresh = await getAdoToken(config.cloudpcHost);
      return await socksPut(url, fresh, body);
    }
    throw err;
  }
}

async function adoSocksDelete(config: ServiceConfig, path: string): Promise<string> {
  const url = buildUrl(path);
  const token = await getAdoToken(config.cloudpcHost);
  try {
    return await socksDelete(url, token);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearTokenCache();
      const fresh = await getAdoToken(config.cloudpcHost);
      return await socksDelete(url, fresh);
    }
    throw err;
  }
}

// ── Tool implementations ───────────────────────────────────────────────

/**
 * Query Azure DevOps work items with optional filters.
 */
export async function adoWorkItems(
  config: ServiceConfig,
  project?: string,
  state?: string,
  type?: string,
  limit: number = 20,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");

  if (!(await isSocksAvailable())) {
    // Original SSH fallback
    const clauses: string[] = [];
    if (project) clauses.push(`[System.TeamProject] = '${sanitize(project)}'`);
    if (state) clauses.push(`[System.State] = '${sanitize(state)}'`);
    if (type) clauses.push(`[System.WorkItemType] = '${sanitize(type)}'`);
    const where = clauses.length > 0 ? ` WHERE ${clauses.join(" AND ")}` : "";
    const wiql = `SELECT [System.Id],[System.Title],[System.State],[System.WorkItemType],[System.AssignedTo] FROM WorkItems${where} ORDER BY [System.ChangedDate] DESC`;
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token; 'Content-Type' = 'application/json' }`,
      `$body = @{ query = '${wiql}' } | ConvertTo-Json`,
      `$r = Invoke-RestMethod -Method POST -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/wit/wiql?api-version=7.0&$top=${limit}' -Headers $h -Body $body`,
      `$r | ConvertTo-Json -Depth 10 -Compress`,
    ].join("; ");
    return sshAdoCommand(config.cloudpcHost, ps, 45_000);
  }

  // SOCKS path: POST WIQL query
  const clauses: string[] = [];
  if (project) clauses.push(`[System.TeamProject] = '${sanitize(project)}'`);
  if (state) clauses.push(`[System.State] = '${sanitize(state)}'`);
  if (type) clauses.push(`[System.WorkItemType] = '${sanitize(type)}'`);
  const where = clauses.length > 0 ? ` WHERE ${clauses.join(" AND ")}` : "";
  const wiql = `SELECT [System.Id],[System.Title],[System.State],[System.WorkItemType],[System.AssignedTo] FROM WorkItems${where} ORDER BY [System.ChangedDate] DESC`;

  return adoSocksPost(
    config,
    `${encodeURIComponent(proj)}/_apis/wit/wiql`,
    { query: wiql },
    { $top: limit },
  );
}

/**
 * List Azure DevOps repositories in a project.
 */
export async function adoRepos(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    let cmd = `az repos list --organization https://dev.azure.com/${ADO_ORG}`;
    if (project) cmd += ` --project '${sanitize(project)}'`;
    cmd += ` -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd);
  }

  const path = project
    ? `${encodeURIComponent(sanitize(project))}/_apis/git/repositories`
    : `_apis/git/repositories`;
  return adoSocksGet(config, path);
}

/**
 * List Azure DevOps pull requests with optional filters.
 */
export async function adoPullRequests(
  config: ServiceConfig,
  project?: string,
  status?: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    let cmd = `az repos pr list --organization https://dev.azure.com/${ADO_ORG}`;
    if (project) cmd += ` --project '${sanitize(project)}'`;
    if (status) cmd += ` --status ${sanitize(status)}`;
    cmd += ` -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd);
  }

  const path = project
    ? `${encodeURIComponent(sanitize(project))}/_apis/git/pullrequests`
    : `_apis/git/pullrequests`;
  const query: Record<string, string> = {};
  if (status) query["searchCriteria.status"] = sanitize(status);
  return adoSocksGet(config, path, query);
}

/**
 * Get details and logs for a specific Azure DevOps build run.
 */
export async function adoBuildLogs(
  config: ServiceConfig,
  buildId: number,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");

  if (!(await isSocksAvailable())) {
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token }`,
      `Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/builds/${buildId}?api-version=7.0' -Headers $h | ConvertTo-Json -Depth 10 -Compress`,
    ].join("; ");
    return sshAdoCommand(config.cloudpcHost, ps);
  }

  return adoSocksGet(config, `${encodeURIComponent(proj)}/_apis/build/builds/${buildId}`);
}

/**
 * Get recent commits from an Azure DevOps Git repository.
 */
export async function adoCommits(
  config: ServiceConfig,
  repoName: string,
  project?: string,
  limit: number = 20,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    const cmd = `az devops invoke --organization https://dev.azure.com/${ADO_ORG} --area git --resource commits --api-version 7.1${project ? ` --route-parameters project='${sanitize(project)}' repositoryId='${sanitize(repoName)}'` : ""} --query-parameters \\$top=${limit} -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd, 45_000);
  }

  const path = project
    ? `${encodeURIComponent(sanitize(project))}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/commits`
    : `_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/commits`;
  return adoSocksGet(config, path, { $top: limit });
}

/**
 * Get pipeline definition details including triggers, variables, and YAML path.
 */
export async function adoPipelineDefinition(
  config: ServiceConfig,
  pipelineId: number,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");

  if (!(await isSocksAvailable())) {
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token }`,
      `Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/pipelines/${pipelineId}?api-version=7.0' -Headers $h | ConvertTo-Json -Depth 10 -Compress`,
    ].join("; ");
    return sshAdoCommand(config.cloudpcHost, ps);
  }

  return adoSocksGet(config, `${encodeURIComponent(proj)}/_apis/pipelines/${pipelineId}`);
}

/**
 * Update a pipeline's default branch or other settings.
 */
export async function adoPipelineUpdate(
  config: ServiceConfig,
  pipelineId: number,
  project: string,
  branch?: string,
): Promise<string> {
  const proj = sanitize(project);

  if (!(await isSocksAvailable())) {
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token; 'Content-Type' = 'application/json' }`,
      `$def = Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/definitions/${pipelineId}?api-version=7.0' -Headers $h`,
      branch ? `$def.repository.defaultBranch = 'refs/heads/${sanitize(branch)}'` : "",
      `$body = $def | ConvertTo-Json -Depth 20 -Compress`,
      `Invoke-RestMethod -Method PUT -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/definitions/${pipelineId}?api-version=7.0' -Headers $h -Body $body | ConvertTo-Json -Depth 10 -Compress`,
    ].filter(Boolean).join("; ");
    return sshAdoCommand(config.cloudpcHost, ps);
  }

  // SOCKS: GET definition, modify, PUT back
  const defRaw = await adoSocksGet(config, `${encodeURIComponent(proj)}/_apis/build/definitions/${pipelineId}`);
  const def = JSON.parse(defRaw);
  if (branch) {
    def.repository = def.repository ?? {};
    def.repository.defaultBranch = `refs/heads/${sanitize(branch)}`;
  }
  return adoSocksPut(config, `${encodeURIComponent(proj)}/_apis/build/definitions/${pipelineId}`, def);
}

/**
 * Set default branch for a repository.
 */
export async function adoRepoUpdate(
  config: ServiceConfig,
  repoName: string,
  project: string,
  defaultBranch: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    const cmd = `az repos update --organization https://dev.azure.com/${ADO_ORG} --repository '${sanitize(repoName)}' --project '${sanitize(project)}' --default-branch '${sanitize(defaultBranch)}' -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd);
  }

  // GET repo first to find its ID, then PATCH
  const reposRaw = await adoSocksGet(config, `${encodeURIComponent(sanitize(project))}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}`);
  const repo = JSON.parse(reposRaw);
  return adoSocksPut(
    config,
    `${encodeURIComponent(sanitize(project))}/_apis/git/repositories/${repo.id}`,
    { defaultBranch: `refs/heads/${sanitize(defaultBranch)}` },
  );
}

/**
 * Run (trigger) a pipeline.
 */
export async function adoPipelineRun(
  config: ServiceConfig,
  pipelineId: number,
  project: string,
  branch?: string,
): Promise<string> {
  const proj = sanitize(project);

  if (!(await isSocksAvailable())) {
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token; 'Content-Type' = 'application/json' }`,
      branch
        ? `$body = @{ resources = @{ repositories = @{ self = @{ refName = 'refs/heads/${sanitize(branch)}' } } } } | ConvertTo-Json -Depth 10 -Compress`
        : `$body = '{ "resources": {} }'`,
      `Invoke-RestMethod -Method POST -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/pipelines/${pipelineId}/runs?api-version=7.0' -Headers $h -Body $body | ConvertTo-Json -Depth 10 -Compress`,
    ].join("; ");
    return sshAdoCommand(config.cloudpcHost, ps);
  }

  const body = branch
    ? { resources: { repositories: { self: { refName: `refs/heads/${sanitize(branch)}` } } } }
    : { resources: {} };
  return adoSocksPost(config, `${encodeURIComponent(proj)}/_apis/pipelines/${pipelineId}/runs`, body);
}

/**
 * List variables in a pipeline.
 */
export async function adoPipelineVariables(
  config: ServiceConfig,
  pipelineId: number,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");

  if (!(await isSocksAvailable())) {
    const ps = [
      `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
      `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
      `$h = @{ Authorization = 'Bearer ' + $token }`,
      `$def = Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/definitions/${pipelineId}?api-version=7.0' -Headers $h`,
      `$def.variables | ConvertTo-Json -Depth 5 -Compress`,
    ].join("; ");
    return sshAdoCommand(config.cloudpcHost, ps);
  }

  const raw = await adoSocksGet(config, `${encodeURIComponent(proj)}/_apis/build/definitions/${pipelineId}`);
  const def = JSON.parse(raw);
  return JSON.stringify(def.variables ?? {}, null, 2);
}

/**
 * List branches in a repository.
 */
export async function adoBranches(
  config: ServiceConfig,
  repoName: string,
  project?: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    let cmd = `az repos ref list --organization https://dev.azure.com/${ADO_ORG} --repository '${sanitize(repoName)}'`;
    if (project) cmd += ` --project '${sanitize(project)}'`;
    cmd += ` --filter heads -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd);
  }

  const path = project
    ? `${encodeURIComponent(sanitize(project))}/_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/refs`
    : `_apis/git/repositories/${encodeURIComponent(sanitize(repoName))}/refs`;
  return adoSocksGet(config, path, { filter: "heads/" });
}

/**
 * Delete a repository (requires confirmation from operator).
 */
export async function adoRepoDelete(
  config: ServiceConfig,
  repoId: string,
  project: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    const cmd = `az repos delete --organization https://dev.azure.com/${ADO_ORG} --id '${sanitize(repoId)}' --project '${sanitize(project)}' --yes -o json 2>$null`;
    return sshAdoCommand(config.cloudpcHost, cmd);
  }

  return adoSocksDelete(config, `${encodeURIComponent(sanitize(project))}/_apis/git/repositories/${encodeURIComponent(sanitize(repoId))}`);
}
