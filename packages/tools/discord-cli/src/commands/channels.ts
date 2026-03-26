import { DiscordClient } from "../auth.js";

interface DiscordChannel {
  id: string;
  name: string;
  type: number;
  position: number;
  parent_id: string | null;
}

// type 0 = text channel, type 4 = category
const TEXT_CHANNEL = 0;
const CATEGORY_CHANNEL = 4;

export async function channelsCommand(
  client: DiscordClient,
  guildId: string,
): Promise<void> {
  const channels = (await client.get(
    `/guilds/${guildId}/channels`,
  )) as DiscordChannel[];

  // Build category ID → name map
  const categoryMap = new Map<string, string>();
  for (const ch of channels) {
    if (ch.type === CATEGORY_CHANNEL) {
      categoryMap.set(ch.id, ch.name);
    }
  }

  // Filter to text channels only
  const textChannels = channels.filter((ch) => ch.type === TEXT_CHANNEL);

  if (textChannels.length === 0) {
    console.log(`No text channels found in guild ${guildId}.`);
    return;
  }

  // Group text channels by category
  const grouped = new Map<string, DiscordChannel[]>();
  const UNCATEGORIZED = "(uncategorized)";

  for (const ch of textChannels) {
    const categoryName = ch.parent_id
      ? (categoryMap.get(ch.parent_id) ?? UNCATEGORIZED)
      : UNCATEGORIZED;
    if (!grouped.has(categoryName)) {
      grouped.set(categoryName, []);
    }
    grouped.get(categoryName)!.push(ch);
  }

  // Sort channels within each category by position
  for (const channelList of grouped.values()) {
    channelList.sort((a, b) => a.position - b.position);
  }

  console.log(`Channels — guild ${guildId} (${textChannels.length} text channels)`);

  for (const [categoryName, chans] of grouped.entries()) {
    console.log(`\n${categoryName}`);
    for (const ch of chans) {
      console.log(`  ${ch.name.padEnd(24)}${ch.id}`);
    }
  }
}
