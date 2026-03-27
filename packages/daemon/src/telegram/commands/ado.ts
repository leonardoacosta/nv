import { fleetGet } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const GRAPH_SVC_PORT = 4107;

/**
 * /ado — Azure DevOps commands via graph-svc
 *
 * Subcommands:
 *   (no args)         — show subcommand help
 *   wi [project]      — query work items
 *   prs [project]     — list pull requests
 *   repos [project]   — list repositories
 */
export async function buildAdoReply(subcommand?: string, arg?: string): Promise<string> {
  if (!subcommand) {
    return [
      "Azure DevOps",
      "─".repeat(32),
      "Usage:",
      "  /ado wi [project]    — work items",
      "  /ado prs [project]   — pull requests",
      "  /ado repos [project] — repositories",
      "",
      "Examples:",
      "  /ado wi",
      "  /ado wi MyProject",
      "  /ado prs MyProject",
    ].join("\n");
  }

  if (subcommand === "wi") {
    return buildWorkItemsReply(arg);
  }

  if (subcommand === "prs") {
    return buildPullRequestsReply(arg);
  }

  if (subcommand === "repos") {
    return buildReposReply(arg);
  }

  return `Unknown subcommand: ${subcommand}\nUsage: /ado, /ado wi, /ado prs, /ado repos`;
}

async function buildWorkItemsReply(project?: string): Promise<string> {
  const params = new URLSearchParams();
  if (project) params.set("project", project);
  params.set("limit", "20");
  const qs = params.toString();

  const data = await fleetGet(GRAPH_SVC_PORT, `/ado/work-items?${qs}`);
  const result = extractResult(data);

  if (typeof result === "string") {
    const label = project ? `Work Items (${project})` : "Work Items";
    return truncate(`${label}\n${"─".repeat(32)}\n${result}`);
  }

  return truncate(
    `Work Items${project ? ` (${project})` : ""}\n${"─".repeat(32)}\n${JSON.stringify(result, null, 2)}`,
  );
}

async function buildPullRequestsReply(project?: string): Promise<string> {
  const params = new URLSearchParams();
  if (project) params.set("project", project);
  params.set("status", "active");
  const qs = params.toString();

  const data = await fleetGet(GRAPH_SVC_PORT, `/ado/pull-requests?${qs}`);
  const result = extractResult(data);

  if (typeof result === "string") {
    const label = project ? `Pull Requests (${project})` : "Pull Requests";
    return truncate(`${label}\n${"─".repeat(32)}\n${result}`);
  }

  return truncate(
    `Pull Requests${project ? ` (${project})` : ""}\n${"─".repeat(32)}\n${JSON.stringify(result, null, 2)}`,
  );
}

async function buildReposReply(project?: string): Promise<string> {
  const params = new URLSearchParams();
  if (project) params.set("project", project);
  const qs = params.toString();

  const data = await fleetGet(GRAPH_SVC_PORT, `/ado/repos${qs ? `?${qs}` : ""}`);
  const result = extractResult(data);

  if (typeof result === "string") {
    const label = project ? `Repos (${project})` : "Repos";
    return truncate(`${label}\n${"─".repeat(32)}\n${result}`);
  }

  return truncate(
    `Repos${project ? ` (${project})` : ""}\n${"─".repeat(32)}\n${JSON.stringify(result, null, 2)}`,
  );
}

function extractResult(data: unknown): unknown {
  if (data && typeof data === "object" && "result" in data) {
    return (data as { result: unknown }).result;
  }
  return data;
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
