import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * List recent Teams chats and DMs.
 *
 * Runs: `graph-teams.ps1 -Action list`
 */
export async function teamsListChats(limit?: number): Promise<string> {
  const _limit = Math.max(1, Math.min(limit ?? 20, 50));
  // The PowerShell script's -Action list doesn't take a limit param,
  // but we clamp for API consistency. Pass through to script as-is.
  void _limit;
  return sshCloudPc(SCRIPT, "-Action list");
}
