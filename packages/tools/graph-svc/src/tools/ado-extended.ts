import type { ServiceConfig } from "../config.js";
import { sshAdoCommand } from "../ssh.js";
import { sanitize } from "../utils.js";

/** Azure DevOps organization URL. */
const ADO_ORG = "https://dev.azure.com/brownandbrowninc";

const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

/**
 * Query Azure DevOps work items with optional filters.
 * Uses WIQL via REST API (POST) to avoid az CLI quoting issues.
 */
export async function adoWorkItems(
  config: ServiceConfig,
  project?: string,
  state?: string,
  type?: string,
  limit: number = 20,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");
  const clauses: string[] = [];
  if (project) clauses.push(`[System.TeamProject] = '${sanitize(project)}'`);
  if (state) clauses.push(`[System.State] = '${sanitize(state)}'`);
  if (type) clauses.push(`[System.WorkItemType] = '${sanitize(type)}'`);

  const where = clauses.length > 0 ? ` WHERE ${clauses.join(" AND ")}` : "";
  const wiql = `SELECT [System.Id],[System.Title],[System.State],[System.WorkItemType],[System.AssignedTo] FROM WorkItems${where} ORDER BY [System.ChangedDate] DESC`;

  // POST WIQL query via REST — avoids all quoting issues
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

/**
 * List Azure DevOps repositories in a project.
 */
export async function adoRepos(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  let cmd = `az repos list --organization ${ADO_ORG}`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  cmd += ` -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}

/**
 * List Azure DevOps pull requests with optional filters.
 */
export async function adoPullRequests(
  config: ServiceConfig,
  project?: string,
  status?: string,
): Promise<string> {
  let cmd = `az repos pr list --organization ${ADO_ORG}`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  if (status) {
    cmd += ` --status ${sanitize(status)}`;
  }
  cmd += ` -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
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
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
    `$h = @{ Authorization = 'Bearer ' + $token }`,
    `Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/builds/${buildId}?api-version=7.0' -Headers $h | ConvertTo-Json -Depth 10 -Compress`,
  ].join("; ");
  return sshAdoCommand(config.cloudpcHost, ps);
}

// ── Git & Release Tools ──────────────────────────────────────────────

/**
 * Get recent commits from an Azure DevOps Git repository.
 * Useful for contributor analysis and activity tracking.
 */
export async function adoCommits(
  config: ServiceConfig,
  repoName: string,
  project?: string,
  limit: number = 20,
): Promise<string> {
  let cmd = `az repos ref list --organization ${ADO_ORG} --repository '${sanitize(repoName)}'`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  // Use the REST API via az devops invoke for commit history
  // az repos doesn't have a direct "commits" command, so we use invoke
  cmd = `az devops invoke --organization ${ADO_ORG} --area git --resource commits --api-version 7.1`;
  if (project) {
    cmd += ` --route-parameters project='${sanitize(project)}' repositoryId='${sanitize(repoName)}'`;
  }
  cmd += ` --query-parameters \\$top=${limit} -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd, 45_000);
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
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
    `$h = @{ Authorization = 'Bearer ' + $token }`,
    `Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/pipelines/${pipelineId}?api-version=7.0' -Headers $h | ConvertTo-Json -Depth 10 -Compress`,
  ].join("; ");
  return sshAdoCommand(config.cloudpcHost, ps);
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
  // Pipeline update uses PATCH on the build definition
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

/**
 * Set default branch for a repository.
 */
export async function adoRepoUpdate(
  config: ServiceConfig,
  repoName: string,
  project: string,
  defaultBranch: string,
): Promise<string> {
  const cmd = `az repos update --organization ${ADO_ORG} --repository '${sanitize(repoName)}' --project '${sanitize(project)}' --default-branch '${sanitize(defaultBranch)}' -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}

/**
 * Run (trigger) a pipeline. Returns the new build run details.
 */
export async function adoPipelineRun(
  config: ServiceConfig,
  pipelineId: number,
  project: string,
  branch?: string,
): Promise<string> {
  const proj = sanitize(project);
  const bodyParts = [`resources = @{}`];
  if (branch) bodyParts.push(`stagesToSkip = @()`);
  // Use the pipelines/runs POST endpoint
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

/**
 * List variables in a pipeline.
 */
export async function adoPipelineVariables(
  config: ServiceConfig,
  pipelineId: number,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");
  // Get build definition which contains variables
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
    `$h = @{ Authorization = 'Bearer ' + $token }`,
    `$def = Invoke-RestMethod -Uri 'https://dev.azure.com/${ADO_ORG}/${proj}/_apis/build/definitions/${pipelineId}?api-version=7.0' -Headers $h`,
    `$def.variables | ConvertTo-Json -Depth 5 -Compress`,
  ].join("; ");
  return sshAdoCommand(config.cloudpcHost, ps);
}

/**
 * List branches in a repository.
 */
export async function adoBranches(
  config: ServiceConfig,
  repoName: string,
  project?: string,
): Promise<string> {
  let cmd = `az repos ref list --organization ${ADO_ORG} --repository '${sanitize(repoName)}'`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  cmd += ` --filter heads -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}

/**
 * Delete a repository (requires confirmation from operator).
 */
export async function adoRepoDelete(
  config: ServiceConfig,
  repoId: string,
  project: string,
): Promise<string> {
  const cmd = `az repos delete --organization ${ADO_ORG} --id '${sanitize(repoId)}' --project '${sanitize(project)}' --yes -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}
