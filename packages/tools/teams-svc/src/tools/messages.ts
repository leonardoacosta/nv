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
}

interface GraphChannelMessage {
  id?: string;
  body?: { contentType?: string; content?: string };
  from?: { user?: { displayName?: string } };
  createdDateTime?: string;
}

function formatChannelMessages(messages: GraphChannelMessage[]): string {
  if (messages.length === 0) return "No messages found.";
  return messages
    .map((msg) => {
      const sender = msg.from?.user?.displayName ?? "Unknown";
      const date = msg.createdDateTime
        ? new Date(msg.createdDateTime).toLocaleString("en-US", {
            month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
          })
        : "";
      let content = msg.body?.content ?? "";
      if (msg.body?.contentType === "html") {
        content = content.replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").trim();
      }
      return `[${date}] ${sender}: ${content}`;
    })
    .join("\n");
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

async function resolveTeamId(teamName: string): Promise<string> {
  const raw = await graphGet(`${GRAPH_BASE}/me/joinedTeams`);
  const data = JSON.parse(raw) as { value?: GraphTeam[] };
  const team = data.value?.find(
    (t) => t.displayName?.toLowerCase() === teamName.toLowerCase(),
  );
  if (!team?.id) throw new Error(`Team not found: ${teamName}`);
  return team.id;
}

async function resolveChannelId(teamId: string, channelName?: string): Promise<string> {
  const raw = await graphGet(`${GRAPH_BASE}/teams/${teamId}/channels`);
  const data = JSON.parse(raw) as { value?: GraphChannel[] };
  if (!channelName) {
    // Default to General
    const general = data.value?.find((c) => c.displayName === "General");
    if (general?.id) return general.id;
    if (data.value?.[0]?.id) return data.value[0].id;
    throw new Error("No channels found");
  }
  const channel = data.value?.find(
    (c) => c.displayName?.toLowerCase() === channelName.toLowerCase(),
  );
  if (!channel?.id) throw new Error(`Channel not found: ${channelName}`);
  return channel.id;
}

/**
 * Read messages from a Teams channel.
 */
export async function teamsMessages(
  teamName: string,
  channelName?: string,
  count?: number,
): Promise<string> {
  if (!teamName.trim()) {
    throw new Error("team_name is required");
  }

  if (!(await isSocksAvailable())) {
    let args = `-Action messages -TeamName '${teamName}'`;
    if (channelName?.trim()) args += ` -ChannelName '${channelName}'`;
    if (count !== undefined) {
      const clamped = Math.max(1, Math.min(count, 50));
      args += ` -Count ${clamped}`;
    }
    return sshCloudPc(SCRIPT, args);
  }

  const top = count ? Math.max(1, Math.min(count, 50)) : 20;
  const teamId = await resolveTeamId(teamName);
  const channelId = await resolveChannelId(teamId, channelName);
  const raw = await graphGet(
    `${GRAPH_BASE}/teams/${teamId}/channels/${channelId}/messages?$top=${top}`,
  );
  const data = JSON.parse(raw) as { value?: GraphChannelMessage[] };
  return formatChannelMessages(data.value ?? []);
}
