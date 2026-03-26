import { DiscordClient } from "../auth.js";
import { relativeTime } from "../utils.js";

interface DiscordMessage {
  id: string;
  type: number;
  content: string;
  timestamp: string;
  author: {
    username: string;
    global_name: string | null;
  };
}

const MAX_CONTENT_LENGTH = 500;
// type 0 = regular message
const USER_MESSAGE = 0;

export async function messagesCommand(
  client: DiscordClient,
  channelId: string,
  limit: number,
): Promise<void> {
  const messages = (await client.get(
    `/channels/${channelId}/messages?limit=${limit}`,
  )) as DiscordMessage[];

  // Filter to regular user messages only (skip system messages)
  const userMessages = messages.filter((m) => m.type === USER_MESSAGE);

  if (userMessages.length === 0) {
    console.log(`No messages found in channel ${channelId}.`);
    return;
  }

  console.log(`Messages — #channel ${channelId} (last ${userMessages.length})`);

  for (const msg of userMessages) {
    const author = msg.author.global_name ?? msg.author.username;
    const when = relativeTime(msg.timestamp);
    const content = truncate(msg.content, MAX_CONTENT_LENGTH);
    console.log(`[${when}] ${author}: ${content}`);
  }
}

function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + "…";
}
