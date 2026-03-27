import { sshCloudPc } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const SCRIPT = "graph-teams.ps1";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

interface GraphChatMessage {
  id?: string;
  body?: { contentType?: string; content?: string };
  from?: { user?: { displayName?: string } };
  createdDateTime?: string;
  messageType?: string;
}

function formatMessages(messages: GraphChatMessage[]): string {
  // Filter out system messages
  const userMessages = messages.filter((m) => m.messageType === "message" || !m.messageType);
  if (userMessages.length === 0) return "No messages found.";
  return userMessages
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

/**
 * Read messages from a specific Teams chat.
 */
export async function teamsReadChat(
  chatId: string,
  limit?: number,
): Promise<string> {
  if (!chatId.trim()) {
    throw new Error("chat_id is required");
  }
  const count = Math.max(1, Math.min(limit ?? 20, 50));

  if (!(await isSocksAvailable())) {
    return sshCloudPc(SCRIPT, `-Action messages -ChatId '${chatId}' -Count ${count}`);
  }

  const url = `${GRAPH_BASE}/me/chats/${encodeURIComponent(chatId)}/messages?$top=${count}`;
  const token = await getO365Token();

  try {
    const raw = await socksGet(url, token);
    const data = JSON.parse(raw) as { value?: GraphChatMessage[] };
    return formatMessages(data.value ?? []);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const fresh = await getO365Token();
      const raw = await socksGet(url, fresh);
      const data = JSON.parse(raw) as { value?: GraphChatMessage[] };
      return formatMessages(data.value ?? []);
    }
    throw err;
  }
}
