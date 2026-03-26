import { sshCloudPc } from "../ssh.js";

const SCRIPT = "graph-teams.ps1";

/**
 * Read messages from a Teams channel.
 *
 * Runs: `graph-teams.ps1 -Action messages -TeamName '<teamName>' [-ChannelName '<channelName>'] [-Count <count>]`
 */
export async function teamsMessages(
  teamName: string,
  channelName?: string,
  count?: number,
): Promise<string> {
  if (!teamName.trim()) {
    throw new Error("team_name is required");
  }

  let args = `-Action messages -TeamName '${teamName}'`;

  if (channelName?.trim()) {
    args += ` -ChannelName '${channelName}'`;
  }

  if (count !== undefined) {
    const clamped = Math.max(1, Math.min(count, 50));
    args += ` -Count ${clamped}`;
  }

  return sshCloudPc(SCRIPT, args);
}
