import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class DiscordAdapter implements ChannelAdapter {
  readonly name: ChannelName = "discord";
  readonly direction: ChannelDirection = "bidirectional";
  private readonly botToken: string | undefined;

  constructor(botToken?: string) {
    this.botToken = botToken;
  }

  status(): ChannelStatus {
    return this.botToken ? "connected" : "disconnected";
  }

  async send(channelId: string, message: string): Promise<void> {
    if (!this.botToken) {
      throw new Error("Discord bot token not configured");
    }

    const url = `https://discord.com/api/v10/channels/${channelId}/messages`;

    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Authorization": `Bot ${this.botToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ content: message }),
    });

    if (!response.ok) {
      const body = await response.text();
      throw new Error(`Discord API error (${response.status}): ${body}`);
    }
  }
}
