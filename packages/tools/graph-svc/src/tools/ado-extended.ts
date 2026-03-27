import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";

const ADO_SCRIPT = "graph-ado.ps1";

/**
 * Sanitize a user-supplied string before passing it to SSH/PowerShell.
 * Strips single quotes, semicolons, backticks, and pipe characters to prevent injection.
 */
function sanitize(value: string): string {
  return value.replace(/[';`|]/g, "");
}

/**
 * Query Azure DevOps work items with optional filters.
 * @param project Project name to filter by
 * @param state Work item state (Active, New, Closed, etc.)
 * @param type Work item type (Bug, Task, User Story, etc.)
 * @param limit Maximum number of results (default 20)
 */
export async function adoWorkItems(
  config: ServiceConfig,
  project?: string,
  state?: string,
  type?: string,
  limit: number = 20,
): Promise<string> {
  let args = "-Action WorkItems";
  if (project) {
    args += ` -Project '${sanitize(project)}'`;
  }
  if (state) {
    args += ` -State '${sanitize(state)}'`;
  }
  if (type) {
    args += ` -Type '${sanitize(type)}'`;
  }
  args += ` -Limit ${limit}`;
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    args,
  );
}

/**
 * List Azure DevOps repositories in a project.
 * @param project Project name to filter by
 */
export async function adoRepos(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  let args = "-Action Repos";
  if (project) {
    args += ` -Project '${sanitize(project)}'`;
  }
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    args,
  );
}

/**
 * List Azure DevOps pull requests with optional filters.
 * @param project Project name to filter by
 * @param status PR status (active, completed, abandoned)
 */
export async function adoPullRequests(
  config: ServiceConfig,
  project?: string,
  status?: string,
): Promise<string> {
  let args = "-Action PullRequests";
  if (project) {
    args += ` -Project '${sanitize(project)}'`;
  }
  if (status) {
    args += ` -Status '${sanitize(status)}'`;
  }
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    args,
  );
}

/**
 * Get details and logs for a specific Azure DevOps build run.
 * @param buildId The build ID to look up
 * @param project Project name (optional)
 */
export async function adoBuildLogs(
  config: ServiceConfig,
  buildId: number,
  project?: string,
): Promise<string> {
  let args = `-Action BuildLogs -BuildId ${buildId}`;
  if (project) {
    args += ` -Project '${sanitize(project)}'`;
  }
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    args,
  );
}
