import { MsGraphClient } from "../auth.js";
import { relativeTime } from "../format.js";

interface ChatMember {
  displayName?: string;
  userId?: string;
}

interface Chat {
  id: string;
  chatType: "oneOnOne" | "group" | "meeting" | "unknownFutureValue";
  topic?: string | null;
  lastMessageReceivedDateTime?: string | null;
  members?: ChatMember[];
}

interface GraphListResponse<T> {
  value: T[];
}

function formatChatLabel(chat: Chat, currentUserId?: string): string {
  if (chat.topic) {
    const prefix =
      chat.chatType === "meeting"
        ? "Meeting"
        : chat.chatType === "group"
          ? "Group"
          : "Chat";
    return `${prefix}: ${chat.topic}`;
  }

  if (chat.chatType === "oneOnOne" && chat.members && chat.members.length > 0) {
    const other = chat.members.find(
      (m) => !currentUserId || m.userId !== currentUserId
    );
    const name = other?.displayName ?? "Unknown";
    return `DM: ${name}`;
  }

  return `Chat: ${chat.id.slice(0, 8)}...`;
}

export async function listChats(limit: number): Promise<void> {
  const client = new MsGraphClient();
  const qs = new URLSearchParams({
    $top: String(Math.min(limit, 50)),
    $expand: "members",
    $orderby: "lastMessageReceivedDateTime desc",
  });

  const data = (await client.get(
    `/chats?${qs.toString()}`
  )) as GraphListResponse<Chat>;

  const chats = data.value ?? [];
  if (chats.length === 0) {
    process.stdout.write("No chats found.\n");
    return;
  }

  process.stdout.write(`Recent Chats (${chats.length})\n`);
  for (const chat of chats) {
    const label = formatChatLabel(chat);
    const when = relativeTime(chat.lastMessageReceivedDateTime);
    process.stdout.write(`${label} — last active ${when}\n`);
  }
}

export async function readChat(chatId: string, limit: number): Promise<void> {
  const client = new MsGraphClient();
  const qs = new URLSearchParams({
    $top: String(Math.min(limit, 50)),
    $orderby: "createdDateTime desc",
  });

  // Get chat details first for the header
  const chatData = (await client.get(`/chats/${chatId}`)) as Chat;
  const label = chatData.topic ?? chatId;

  const data = (await client.get(
    `/chats/${chatId}/messages?${qs.toString()}`
  )) as GraphListResponse<{
    id: string;
    createdDateTime?: string;
    from?: { user?: { displayName?: string } };
    body?: { content?: string; contentType?: string };
  }>;

  const messages = (data.value ?? []).reverse();
  if (messages.length === 0) {
    process.stdout.write(`Chat: ${label} — no messages\n`);
    return;
  }

  process.stdout.write(`Chat: ${label} (last ${messages.length} messages)\n`);
  for (const msg of messages) {
    const sender = msg.from?.user?.displayName ?? "Unknown";
    const when = relativeTime(msg.createdDateTime);
    const rawBody = msg.body?.content ?? "";
    const text =
      msg.body?.contentType === "html"
        ? (await import("../format.js")).stripHtml(rawBody)
        : rawBody.replace(/\s+/g, " ").trim();
    process.stdout.write(`[${when}] ${sender}: ${text}\n`);
  }
}
