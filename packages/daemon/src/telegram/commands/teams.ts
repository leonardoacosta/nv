import { fleetGet } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const TEAMS_SVC_PORT = 4105;

interface TeamChat {
  id: string;
  name?: string;
  topic?: string;
  chatType?: string;
}

/**
 * /teams — list recent Teams chats
 */
export async function buildTeamsReply(): Promise<string> {
  const data = await fleetGet(TEAMS_SVC_PORT, "/chats");
  const chats = (
    Array.isArray(data)
      ? data
      : Array.isArray((data as { chats?: unknown }).chats)
        ? (data as { chats: unknown[] }).chats
        : []
  ) as TeamChat[];

  if (chats.length === 0) {
    return "No Teams chats found.";
  }

  const header = `Teams Chats (${chats.length})\n${"─".repeat(32)}\n`;
  const lines = chats.map((c) => {
    const name = c.name ?? c.id;
    const type = c.chatType ? ` [${c.chatType}]` : "";
    return `  ${name}${type}`;
  });

  return truncate(header + lines.join("\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
