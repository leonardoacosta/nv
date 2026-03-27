import { sshCloudPc } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const SCRIPT = "graph-teams.ps1";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

interface GraphChat {
  id?: string;
  topic?: string;
  chatType?: string;
  lastMessagePreview?: {
    body?: { content?: string };
    from?: { user?: { displayName?: string } };
    createdDateTime?: string;
  };
  members?: Array<{ displayName?: string }>;
}

function formatChats(chats: GraphChat[]): string {
  if (chats.length === 0) return "No chats found.";
  return chats
    .map((chat, i) => {
      const topic = chat.topic || chat.chatType || "Chat";
      const lastMsg = chat.lastMessagePreview;
      const preview = lastMsg?.body?.content
        ? `\n  Last: ${lastMsg.from?.user?.displayName ?? "Unknown"}: ${lastMsg.body.content.slice(0, 100)}`
        : "";
      const date = lastMsg?.createdDateTime
        ? ` (${new Date(lastMsg.createdDateTime).toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })})`
        : "";
      return `${i + 1}. ${topic}${date}${preview}\n  ID: ${chat.id ?? ""}`;
    })
    .join("\n\n");
}

/**
 * List recent Teams chats and DMs.
 */
export async function teamsListChats(limit?: number): Promise<string> {
  const _limit = Math.max(1, Math.min(limit ?? 20, 50));

  if (!(await isSocksAvailable())) {
    return sshCloudPc(SCRIPT, "-Action list");
  }

  const url = `${GRAPH_BASE}/me/chats?$expand=lastMessagePreview&$top=${_limit}&$orderby=lastMessagePreview/createdDateTime desc`;
  const token = await getO365Token();

  try {
    const raw = await socksGet(url, token);
    const data = JSON.parse(raw) as { value?: GraphChat[] };
    return formatChats(data.value ?? []);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const fresh = await getO365Token();
      const raw = await socksGet(url, fresh);
      const data = JSON.parse(raw) as { value?: GraphChat[] };
      return formatChats(data.value ?? []);
    }
    throw err;
  }
}
