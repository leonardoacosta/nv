import type { ChannelName, ChannelStatus, ChannelDirection, ChannelInfo } from "../types.js";
export interface ChannelAdapter {
    readonly name: ChannelName;
    readonly direction: ChannelDirection;
    status(): ChannelStatus;
    send(target: string, message: string): Promise<void>;
}
export declare class AdapterRegistry {
    private adapters;
    register(adapter: ChannelAdapter): void;
    get(name: ChannelName): ChannelAdapter | undefined;
    list(): ChannelInfo[];
    has(name: ChannelName): boolean;
}
