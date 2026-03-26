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
 * List Azure DevOps projects.
 */
export async function adoProjects(config: ServiceConfig): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    "-Action Projects",
  );
}

/**
 * List Azure DevOps pipelines, optionally filtered by project.
 */
export async function adoPipelines(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  let args = "-Action Pipelines";
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
 * Get recent Azure DevOps builds, optionally filtered by project, pipeline, and limited.
 */
export async function adoBuilds(
  config: ServiceConfig,
  project?: string,
  pipeline?: string,
  limit: number = 10,
): Promise<string> {
  let args = "-Action Builds";
  if (project) {
    args += ` -Project '${sanitize(project)}'`;
  }
  if (pipeline) {
    args += ` -Pipeline '${sanitize(pipeline)}'`;
  }
  args += ` -Limit ${limit}`;
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    ADO_SCRIPT,
    args,
  );
}
