import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * Get a user's Teams presence/availability status.
 *
 * Runs: `graph-teams.ps1 -Action presence -User '<user>'`
 */
export async function teamsPresence(user: string): Promise<string> {
  if (!user.trim()) {
    throw new Error("user is required");
  }
  return sshCloudPc(SCRIPT, `-Action presence -User '${user}'`);
}
