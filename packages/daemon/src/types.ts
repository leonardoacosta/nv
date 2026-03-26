export type Channel = "telegram" | "teams" | "discord" | "email" | "imessage";

export interface Message {
  id: string;
  channel: Channel;
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
