import type { ServiceConfig } from "../config.js";
import { sshAdoCommand } from "../ssh.js";
import { sanitize } from "../utils.js";

/** Azure DevOps organization URL. */
const ADO_ORG = "https://dev.azure.com/brownandbrowninc";

/**
 * List Azure DevOps projects.
 */
export async function adoProjects(config: ServiceConfig): Promise<string> {
  const cmd = `az devops project list --organization ${ADO_ORG} -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}

/**
 * List Azure DevOps pipelines, optionally filtered by project.
 */
export async function adoPipelines(
  config: ServiceConfig,
  project?: string,
): Promise<string> {
  let cmd = `az pipelines list --organization ${ADO_ORG}`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  cmd += ` -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
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
  let cmd = `az pipelines runs list --organization ${ADO_ORG}`;
  if (project) {
    cmd += ` --project '${sanitize(project)}'`;
  }
  if (pipeline) {
    cmd += ` --pipeline-ids (az pipelines list --organization ${ADO_ORG}`;
    if (project) {
      cmd += ` --project '${sanitize(project)}'`;
    }
    cmd += ` --name '${sanitize(pipeline)}' --query '[0].id' -o tsv 2>$null)`;
  }
  cmd += ` --top ${limit} -o json 2>$null`;
  return sshAdoCommand(config.cloudpcHost, cmd);
}
