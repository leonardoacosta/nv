import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class IMessageAdapter implements ChannelAdapter {
  readonly name: ChannelName = "imessage";
  readonly direction: ChannelDirection = "bidirectional";

  status(): ChannelStatus {
    return "disconnected";
  }

  async send(_target: string, _message: string): Promise<void> {
    throw new Error("iMessage adapter not yet implemented");
  }
}
