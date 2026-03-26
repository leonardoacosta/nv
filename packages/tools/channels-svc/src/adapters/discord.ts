import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class DiscordAdapter implements ChannelAdapter {
  readonly name: ChannelName = "discord";
  readonly direction: ChannelDirection = "bidirectional";

  status(): ChannelStatus {
    return "disconnected";
  }

  async send(_target: string, _message: string): Promise<void> {
    throw new Error("Discord adapter not yet implemented");
  }
}
