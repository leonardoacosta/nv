import { MsGraphClient } from "../auth.js";

interface Channel {
  id: string;
  displayName: string;
  description?: string | null;
  membershipType?: string;
}

interface GraphListResponse<T> {
  value: T[];
}

export async function listChannels(teamId: string): Promise<void> {
  const client = new MsGraphClient();
  const data = (await client.get(
    `/teams/${teamId}/channels`
  )) as GraphListResponse<Channel>;

  const channels = data.value ?? [];
  if (channels.length === 0) {
    process.stdout.write(`No channels found in team ${teamId}.\n`);
    return;
  }

  process.stdout.write(`Channels in team ${teamId} (${channels.length})\n`);
  for (const ch of channels) {
    const desc = ch.description ? ` — ${ch.description}` : "";
    const type = ch.membershipType ? ` [${ch.membershipType}]` : "";
    process.stdout.write(`${ch.displayName}${type}${desc}\n`);
    process.stdout.write(`  id: ${ch.id}\n`);
  }
}
