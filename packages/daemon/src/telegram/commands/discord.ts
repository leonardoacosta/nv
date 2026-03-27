import { fleetGet } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const DISCORD_SVC_PORT = 4104;

interface Guild {
  id: string;
  name?: string;
  memberCount?: number;
}

/**
 * /discord — list Discord guilds (servers)
 */
export async function buildDiscordReply(): Promise<string> {
  const data = await fleetGet(DISCORD_SVC_PORT, "/guilds");
  const guilds = (
    Array.isArray(data)
      ? data
      : Array.isArray((data as { guilds?: unknown }).guilds)
        ? (data as { guilds: unknown[] }).guilds
        : []
  ) as Guild[];

  if (guilds.length === 0) {
    return "No Discord servers found.";
  }

  const header = `Discord Servers (${guilds.length})\n${"─".repeat(32)}\n`;
  const lines = guilds.map((g) => {
    const name = g.name ?? g.id;
    const members = g.memberCount ? ` (${g.memberCount} members)` : "";
    return `  ${name}${members}`;
  });

  return truncate(header + lines.join("\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
