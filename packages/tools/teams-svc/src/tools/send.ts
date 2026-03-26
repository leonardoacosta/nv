import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * Send a message to a Teams chat.
 *
 * Runs: `graph-teams.ps1 -Action send -ChatId '<chatId>' -Message '<message>'`
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
  return sshCloudPc(SCRIPT, `-Action send -ChatId '${chatId}' -Message '${message}'`);
}
