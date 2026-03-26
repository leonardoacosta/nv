import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class TeamsAdapter implements ChannelAdapter {
  readonly name: ChannelName = "teams";
  readonly direction: ChannelDirection = "bidirectional";

  status(): ChannelStatus {
    return "disconnected";
  }

  async send(_target: string, _message: string): Promise<void> {
    throw new Error("Teams adapter not yet implemented");
  }
}
