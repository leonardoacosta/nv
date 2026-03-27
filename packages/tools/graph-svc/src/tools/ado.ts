import type { ServiceConfig } from "../config.js";
import { sshAdoCommand } from "../ssh.js";
import { sanitize } from "../utils.js";

/** Azure DevOps organization. */
const ADO_ORG = "brownandbrowninc";
const ADO_BASE = `https://dev.azure.com/${ADO_ORG}`;
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

/**
 * Build a PowerShell snippet that acquires an ADO token and calls Invoke-RestMethod.
 * Runs entirely on CloudPC — no quoting issues, no az CLI extensions needed.
 */
function adoRestCall(apiPath: string, queryParams: string = ""): string {
  const url = `${ADO_BASE}/${apiPath}`;
  const fullUrl = queryParams ? `${url}?${queryParams}&api-version=7.0` : `${url}?api-version=7.0`;
  return [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Token failed'; exit 1 }`,
    `$h = @{ Authorization = 'Bearer ' + $token }`,
    `Invoke-RestMethod -Uri '${fullUrl}' -Headers $h | ConvertTo-Json -Depth 10 -Compress`,
  ].join("; ");
}

/**
 * List Azure DevOps projects.
 */
export async function adoProjects(config: ServiceConfig): Promise<string> {
  const ps = adoRestCall("_apis/projects");
  return sshAdoCommand(config.cloudpcHost, ps);
}

/**
 * List Azure DevOps pipelines, optionally filtered by project.
 */
export async function adoPipelines(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");
  const ps = adoRestCall(`${proj}/_apis/pipelines`);
  return sshAdoCommand(config.cloudpcHost, ps);
}

/**
 * Get recent Azure DevOps builds/runs, optionally filtered by project.
 */
export async function adoBuilds(
  config: ServiceConfig,
  project?: string,
  pipeline?: string,
  limit: number = 10,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");
  let query = `$top=${limit}`;
  if (pipeline) {
    query += `&definitions=${sanitize(pipeline)}`;
  }
  const ps = adoRestCall(`${proj}/_apis/build/builds`, query);
  return sshAdoCommand(config.cloudpcHost, ps);
}
