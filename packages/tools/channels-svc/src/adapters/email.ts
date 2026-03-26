import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";

export class EmailAdapter implements ChannelAdapter {
  readonly name: ChannelName = "email";
  readonly direction: ChannelDirection = "outbound";

  status(): ChannelStatus {
    return "disconnected";
  }

  async send(_target: string, _message: string): Promise<void> {
    throw new Error("Email adapter not yet implemented");
  }
}
