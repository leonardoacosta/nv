import type { ServiceConfig } from "../config.js";
import { sshAdoCommand } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getAdoToken, clearTokenCache } from "./ado-rest.js";
import { sanitize } from "../utils.js";

/** Azure DevOps organization. */
const ADO_ORG = "brownandbrowninc";
const ADO_BASE = `https://dev.azure.com/${ADO_ORG}`;
const API_VERSION = "7.0";
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";

// ── Helpers ────────────────────────────────────────────────────────────

async function adoGet(
  config: ServiceConfig,
  path: string,
  query?: Record<string, string>,
): Promise<string> {
  const url = new URL(`${ADO_BASE}/${path}`);
  url.searchParams.set("api-version", API_VERSION);
  if (query) {
    for (const [k, v] of Object.entries(query)) {
      url.searchParams.set(k, v);
    }
  }

  const token = await getAdoToken(config.cloudpcHost);
  try {
    return await socksGet(url.toString(), token);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearTokenCache();
      const freshToken = await getAdoToken(config.cloudpcHost);
      return await socksGet(url.toString(), freshToken);
    }
    throw err;
  }
}

/**
 * Build a PowerShell snippet that acquires an ADO token and calls Invoke-RestMethod.
 * Used as SSH fallback only.
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

// ── Tool implementations ───────────────────────────────────────────────

/**
 * List Azure DevOps projects.
 */
export async function adoProjects(config: ServiceConfig): Promise<string> {
  if (!(await isSocksAvailable())) {
    const ps = adoRestCall("_apis/projects");
    return sshAdoCommand(config.cloudpcHost, ps);
  }
  return adoGet(config, "_apis/projects");
}

/**
 * List Azure DevOps pipelines, optionally filtered by project.
 */
export async function adoPipelines(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  const proj = sanitize(project ?? "Wholesale Architecture");
  if (!(await isSocksAvailable())) {
    const ps = adoRestCall(`${proj}/_apis/pipelines`);
    return sshAdoCommand(config.cloudpcHost, ps);
  }
  return adoGet(config, `${encodeURIComponent(proj)}/_apis/pipelines`);
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
  if (!(await isSocksAvailable())) {
    let query = `$top=${limit}`;
    if (pipeline) {
      query += `&definitions=${sanitize(pipeline)}`;
    }
    const ps = adoRestCall(`${proj}/_apis/build/builds`, query);
    return sshAdoCommand(config.cloudpcHost, ps);
  }
  const query: Record<string, string> = { $top: String(limit) };
  if (pipeline) query["definitions"] = sanitize(pipeline);
  return adoGet(config, `${encodeURIComponent(proj)}/_apis/build/builds`, query);
}
