import { sshCloudPc } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const SCRIPT = "graph-teams.ps1";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

interface GraphPresence {
  availability?: string;
  activity?: string;
  statusMessage?: { message?: { content?: string } };
}

/**
 * Get a user's Teams presence/availability status.
 */
export async function teamsPresence(user: string): Promise<string> {
  if (!user.trim()) {
    throw new Error("user is required");
  }

  if (!(await isSocksAvailable())) {
    return sshCloudPc(SCRIPT, `-Action presence -User '${user}'`);
  }

  const url = `${GRAPH_BASE}/users/${encodeURIComponent(user)}/presence`;
  const token = await getO365Token();

  let raw: string;
  try {
    raw = await socksGet(url, token);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const fresh = await getO365Token();
      raw = await socksGet(url, fresh);
    } else {
      throw err;
    }
  }

  const presence = JSON.parse(raw) as GraphPresence;
  const status = presence.availability ?? "Unknown";
  const activity = presence.activity ? ` (${presence.activity})` : "";
  const statusMsg = presence.statusMessage?.message?.content
    ? `\nStatus: ${presence.statusMessage.message.content}`
    : "";
  return `${user}: ${status}${activity}${statusMsg}`;
}
