import type { DiscordClient } from "../client.js";

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
// type 0 = regular user message
const USER_MESSAGE = 0;

export interface MessagesResult {
  channel_id: string;
  messages: Array<{
    id: string;
    author: string;
    content: string;
    timestamp: string;
  }>;
}

export async function readMessages(
  client: DiscordClient,
  channelId: string,
  limit: number,
): Promise<MessagesResult> {
  const clampedLimit = Math.max(1, Math.min(100, limit));

  const messages = (await client.get(
    `/channels/${channelId}/messages?limit=${clampedLimit}`,
  )) as DiscordMessage[];

  // Filter to regular user messages only (skip system messages)
  const userMessages = messages.filter((m) => m.type === USER_MESSAGE);

  return {
    channel_id: channelId,
    messages: userMessages.map((msg) => ({
      id: msg.id,
      author: msg.author.global_name ?? msg.author.username,
      content: truncate(msg.content, MAX_CONTENT_LENGTH),
      timestamp: msg.timestamp,
    })),
  };
}

function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + "...";
}
