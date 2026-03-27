import type { ChannelAdapter } from "./registry.js";
import type { ChannelName, ChannelDirection, ChannelStatus } from "../types.js";
export declare class DiscordAdapter implements ChannelAdapter {
    readonly name: ChannelName;
    readonly direction: ChannelDirection;
    private readonly botToken;
    constructor(botToken?: string);
    status(): ChannelStatus;
    send(channelId: string, message: string): Promise<void>;
}
