import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * Read messages from a specific Teams chat (DM or group chat).
 *
 * Runs: `graph-teams.ps1 -Action messages -ChatId '<chatId>' -Count <limit>`
 */
export async function teamsReadChat(
  chatId: string,
  limit?: number,
): Promise<string> {
  if (!chatId.trim()) {
    throw new Error("chat_id is required");
  }
  const count = Math.max(1, Math.min(limit ?? 20, 50));
  return sshCloudPc(SCRIPT, `-Action messages -ChatId '${chatId}' -Count ${count}`);
}
