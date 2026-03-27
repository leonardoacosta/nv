export type Channel = "telegram" | "teams" | "discord" | "email" | "imessage" | "dashboard";

export type MessageType = "text" | "voice" | "photo" | "callback";

export interface Message {
  id: string;
  channel: Channel;
  // New fields for Telegram adapter (and future channel adapters)
  chatId: string;
  text: string;
  type: MessageType;
  from: {
    id: string;
    username?: string;
    firstName: string;
  };
  timestamp: Date;
  metadata: Record<string, unknown>;
  // Legacy fields — kept for backward compatibility with existing code
  threadId?: string;
  senderId: string;
  senderName: string;
  content: string;
  receivedAt: Date;
}

export interface Trigger {
  id: string;
  pattern: string; // regex or keyword
  channel?: Channel; // undefined = all channels
  description: string;
}

export interface Obligation {
  id: string;
  description: string;
  sourceMessageId?: string;
  channel?: Channel;
  dueAt?: Date;
  createdAt: Date;
  status: "pending" | "in_progress" | "done" | "cancelled";
}
