import type { DiscordClient } from "../client.js";

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

const UNCATEGORIZED = "(uncategorized)";

export interface ChannelsResult {
  guild_id: string;
  channels: Array<{
    id: string;
    name: string;
    category: string;
    position: number;
  }>;
}

export async function listChannels(
  client: DiscordClient,
  guildId: string,
): Promise<ChannelsResult> {
  const channels = (await client.get(
    `/guilds/${guildId}/channels`,
  )) as DiscordChannel[];

  // Build category ID -> name map
  const categoryMap = new Map<string, string>();
  for (const ch of channels) {
    if (ch.type === CATEGORY_CHANNEL) {
      categoryMap.set(ch.id, ch.name);
    }
  }

  // Filter to text channels, resolve category names, sort by category then position
  const textChannels = channels
    .filter((ch) => ch.type === TEXT_CHANNEL)
    .map((ch) => ({
      id: ch.id,
      name: ch.name,
      category: ch.parent_id
        ? (categoryMap.get(ch.parent_id) ?? UNCATEGORIZED)
        : UNCATEGORIZED,
      position: ch.position,
    }));

  // Group by category and sort by position within each group
  const grouped = new Map<string, typeof textChannels>();
  for (const ch of textChannels) {
    if (!grouped.has(ch.category)) {
      grouped.set(ch.category, []);
    }
    grouped.get(ch.category)!.push(ch);
  }

  const sorted: typeof textChannels = [];
  for (const channelList of grouped.values()) {
    channelList.sort((a, b) => a.position - b.position);
    sorted.push(...channelList);
  }

  return {
    guild_id: guildId,
    channels: sorted,
  };
}
