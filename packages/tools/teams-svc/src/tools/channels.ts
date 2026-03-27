import { sshCloudPc } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const SCRIPT = "graph-teams.ps1";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

interface GraphTeam {
  id?: string;
  displayName?: string;
}

interface GraphChannel {
  id?: string;
  displayName?: string;
  description?: string;
  membershipType?: string;
}

function formatChannels(channels: GraphChannel[]): string {
  if (channels.length === 0) return "No channels found.";
  return channels
    .map((ch, i) => {
      const desc = ch.description ? `\n  ${ch.description}` : "";
      const type = ch.membershipType ? ` (${ch.membershipType})` : "";
      return `${i + 1}. ${ch.displayName ?? "Unknown"}${type}${desc}\n  ID: ${ch.id ?? ""}`;
    })
    .join("\n\n");
}

async function graphGet(url: string): Promise<string> {
  const token = await getO365Token();
  try {
    return await socksGet(url, token);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const fresh = await getO365Token();
      return await socksGet(url, fresh);
    }
    throw err;
  }
}

/**
 * List channels in a Teams team.
 */
export async function teamsChannels(teamName: string): Promise<string> {
  if (!teamName.trim()) {
    throw new Error("team_name is required");
  }

  if (!(await isSocksAvailable())) {
    return sshCloudPc(SCRIPT, `-Action channels -TeamName '${teamName}'`);
  }

  // Resolve team name to ID
  const teamsRaw = await graphGet(`${GRAPH_BASE}/me/joinedTeams`);
  const teams = JSON.parse(teamsRaw) as { value?: GraphTeam[] };
  const team = teams.value?.find(
    (t) => t.displayName?.toLowerCase() === teamName.toLowerCase(),
  );
  if (!team?.id) throw new Error(`Team not found: ${teamName}`);

  const raw = await graphGet(`${GRAPH_BASE}/teams/${team.id}/channels`);
  const data = JSON.parse(raw) as { value?: GraphChannel[] };
  return formatChannels(data.value ?? []);
}
