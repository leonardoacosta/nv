import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class TelegramAdapter implements ChannelAdapter {
  readonly name: ChannelName = "telegram";
  readonly direction: ChannelDirection = "bidirectional";
  private readonly botToken: string | undefined;

  constructor(botToken?: string) {
    this.botToken = botToken;
  }

  status(): ChannelStatus {
    return this.botToken ? "connected" : "disconnected";
  }

  async send(chatId: string, message: string): Promise<void> {
    if (!this.botToken) {
      throw new Error("Telegram bot token not configured");
    }

    const url = `https://api.telegram.org/bot${this.botToken}/sendMessage`;

    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        text: message,
      }),
    });

    if (!response.ok) {
      const body = await response.text();
      throw new Error(
        `Telegram API error (${response.status}): ${body}`,
      );
    }
  }
}
