import { fleetGet, fleetPost } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const GRAPH_SVC_PORT = 4107;

/**
 * /pim — PIM role status and activation via graph-svc
 *
 * Subcommands:
 *   (no args)   — show PIM role status
 *   all          — activate all PIM roles
 *   <N>          — activate role by number
 */
export async function buildPimReply(argsText?: string): Promise<string> {
  if (!argsText) {
    return buildPimStatusReply();
  }

  if (argsText === "all") {
    return buildPimActivateAllReply();
  }

  const roleNumber = parseInt(argsText, 10);
  if (!isNaN(roleNumber)) {
    return buildPimActivateReply(roleNumber);
  }

  return [
    "PIM - Azure Privileged Identity Management",
    "─".repeat(32),
    "Usage:",
    "  /pim         — show role status",
    "  /pim <N>     — activate role by number",
    "  /pim all     — activate all roles",
  ].join("\n");
}

async function buildPimStatusReply(): Promise<string> {
  const data = await fleetGet(GRAPH_SVC_PORT, "/pim/status");
  const result = extractResult(data);

  if (typeof result === "string") {
    return truncate(`PIM Roles\n${"─".repeat(32)}\n${result}`);
  }

  return truncate(`PIM Roles\n${"─".repeat(32)}\n${JSON.stringify(result, null, 2)}`);
}

async function buildPimActivateReply(roleNumber: number): Promise<string> {
  const data = await fleetPost(GRAPH_SVC_PORT, "/pim/activate", {
    role_number: roleNumber,
  }) as { result?: string; error?: string };

  if (data.error) {
    return `PIM Activation Error\n${"─".repeat(32)}\n${data.error}`;
  }

  const result = data.result ?? "Activation requested";
  return truncate(`PIM Activate Role #${roleNumber}\n${"─".repeat(32)}\n${result}`);
}

async function buildPimActivateAllReply(): Promise<string> {
  const data = await fleetPost(GRAPH_SVC_PORT, "/pim/activate-all", {}) as {
    result?: string;
    error?: string;
  };

  if (data.error) {
    return `PIM Activate All Error\n${"─".repeat(32)}\n${data.error}`;
  }

  const result = data.result ?? "All roles activation requested";
  return truncate(`PIM Activate All\n${"─".repeat(32)}\n${result}`);
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
