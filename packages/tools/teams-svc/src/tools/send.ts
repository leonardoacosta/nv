import { sshCloudPc } from "../ssh.js";
import { socksPost, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const SCRIPT = "graph-teams.ps1";
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

/**
 * Send a message to a Teams chat.
 */
export async function teamsSend(
  chatId: string,
  message: string,
): Promise<string> {
  if (!chatId.trim()) {
    throw new Error("chat_id is required");
  }
  if (!message.trim()) {
    throw new Error("message is required");
  }

  if (!(await isSocksAvailable())) {
    return sshCloudPc(SCRIPT, `-Action send -ChatId '${chatId}' -Message '${message}'`);
  }

  const url = `${GRAPH_BASE}/me/chats/${encodeURIComponent(chatId)}/messages`;
  const body = { body: { content: message } };
  const token = await getO365Token();

  try {
    const raw = await socksPost(url, token, body);
    const result = JSON.parse(raw) as { id?: string };
    return `Message sent (ID: ${result.id ?? "unknown"})`;
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const fresh = await getO365Token();
      const raw = await socksPost(url, fresh, body);
      const result = JSON.parse(raw) as { id?: string };
      return `Message sent (ID: ${result.id ?? "unknown"})`;
    }
    throw err;
  }
}
