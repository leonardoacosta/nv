import { MsGraphClient } from "../auth.js";
import { relativeTime, stripHtml } from "../format.js";

interface Message {
  id: string;
  createdDateTime?: string;
  from?: {
    user?: { displayName?: string };
    application?: { displayName?: string };
  };
  body?: {
    content?: string;
    contentType?: "text" | "html";
  };
  messageType?: string;
}

interface GraphListResponse<T> {
  value: T[];
}

export async function listMessages(
  teamId: string,
  channelId: string,
  limit: number
): Promise<void> {
  const client = new MsGraphClient();
  const qs = new URLSearchParams({
    $top: String(Math.min(limit, 50)),
  });

  const data = (await client.get(
    `/teams/${teamId}/channels/${channelId}/messages?${qs.toString()}`
  )) as GraphListResponse<Message>;

  // Filter to only actual messages (skip system messages)
  const messages = (data.value ?? [])
    .filter((m) => m.messageType === "message" || !m.messageType)
    .reverse();

  if (messages.length === 0) {
    process.stdout.write(`No messages found in channel ${channelId}.\n`);
    return;
  }

  process.stdout.write(
    `Channel messages (last ${messages.length})\n`
  );
  for (const msg of messages) {
    const sender =
      msg.from?.user?.displayName ??
      msg.from?.application?.displayName ??
      "Unknown";
    const when = relativeTime(msg.createdDateTime);
    const rawBody = msg.body?.content ?? "";
    const text =
      msg.body?.contentType === "html"
        ? stripHtml(rawBody)
        : rawBody.replace(/\s+/g, " ").trim();
    process.stdout.write(`[${when}] ${sender}: ${text}\n`);
  }
}
