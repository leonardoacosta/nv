import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";
export declare class IMessageAdapter implements ChannelAdapter {
    readonly name: ChannelName;
    readonly direction: ChannelDirection;
    status(): ChannelStatus;
    send(_target: string, _message: string): Promise<void>;
}
