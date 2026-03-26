import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * List channels in a Teams team.
 *
 * Runs: `graph-teams.ps1 -Action channels -TeamName '<teamName>'`
 */
export async function teamsChannels(teamName: string): Promise<string> {
  if (!teamName.trim()) {
    throw new Error("team_name is required");
  }
  return sshCloudPc(SCRIPT, `-Action channels -TeamName '${teamName}'`);
}
