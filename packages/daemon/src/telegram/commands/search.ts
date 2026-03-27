import { fleetPost } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const MESSAGES_SVC_PORT = 4102;

interface SearchResult {
  sender: string;
  channel: string;
  timestamp: string;
  content: string;
}

/**
 * /search [query] — search messages via messages-svc
 */
export async function buildSearchReply(query?: string): Promise<string> {
  if (!query) {
    return "Usage: /search [query]\nExample: /search meeting tomorrow";
  }

  const data = await fleetPost(MESSAGES_SVC_PORT, "/search", { query });
  const results = (
    Array.isArray(data)
      ? data
      : Array.isArray((data as { results?: unknown }).results)
        ? (data as { results: unknown[] }).results
        : []
  ) as SearchResult[];

  if (results.length === 0) {
    return `No messages found for: ${query}`;
  }

  const header = `Search: "${query}" (${results.length} results)\n${"─".repeat(32)}\n`;
  const lines = results.map((r) => {
    const time = r.timestamp
      ? new Date(r.timestamp).toISOString().slice(0, 16).replace("T", " ")
      : "unknown";
    const preview =
      (r.content ?? "").length > 100
        ? (r.content ?? "").slice(0, 97) + "..."
        : (r.content ?? "");
    return `[${time}] ${r.sender ?? "?"} (${r.channel ?? "?"})\n  ${preview}`;
  });

  return truncate(header + lines.join("\n\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
