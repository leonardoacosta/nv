export type ChannelName =
  | "telegram"
  | "discord"
  | "teams"
  | "email"
  | "imessage";

export type ChannelStatus = "connected" | "disconnected" | "error";

export type ChannelDirection = "inbound" | "outbound" | "bidirectional";

export interface ChannelInfo {
  name: ChannelName;
  status: ChannelStatus;
  direction: ChannelDirection;
}

export interface SendRequest {
  channel: string;
  target: string;
  message: string;
}

export interface SendResult {
  ok: boolean;
  channel?: string;
  target?: string;
  error?: string;
}
