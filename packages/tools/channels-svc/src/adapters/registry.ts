import type {
  ChannelName,
  ChannelStatus,
  ChannelDirection,
  ChannelInfo,
} from "../types.js";

export interface ChannelAdapter {
  readonly name: ChannelName;
  readonly direction: ChannelDirection;
  status(): ChannelStatus;
  send(target: string, message: string): Promise<void>;
}

export class AdapterRegistry {
  private adapters = new Map<ChannelName, ChannelAdapter>();

  register(adapter: ChannelAdapter): void {
    this.adapters.set(adapter.name, adapter);
  }

  get(name: ChannelName): ChannelAdapter | undefined {
    return this.adapters.get(name);
  }

  list(): ChannelInfo[] {
    return Array.from(this.adapters.values()).map((adapter) => ({
      name: adapter.name,
      status: adapter.status(),
      direction: adapter.direction,
    }));
  }

  has(name: ChannelName): boolean {
    return this.adapters.has(name);
  }
}
