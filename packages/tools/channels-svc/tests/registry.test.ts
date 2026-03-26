import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { AdapterRegistry } from "../src/adapters/registry.js";
import type { ChannelAdapter } from "../src/adapters/registry.js";
import type {
  ChannelName,
  ChannelDirection,
  ChannelStatus,
} from "../src/types.js";

function createMockAdapter(
  name: ChannelName,
  direction: ChannelDirection = "bidirectional",
  adapterStatus: ChannelStatus = "connected",
): ChannelAdapter {
  return {
    name,
    direction,
    status: () => adapterStatus,
    send: async () => {},
  };
}

describe("AdapterRegistry", () => {
  it("register and get adapter by name", () => {
    const registry = new AdapterRegistry();
    const adapter = createMockAdapter("telegram");
    registry.register(adapter);

    const result = registry.get("telegram");
    assert.equal(result, adapter);
  });

  it("get returns undefined for unregistered adapter", () => {
    const registry = new AdapterRegistry();
    const result = registry.get("discord");
    assert.equal(result, undefined);
  });

  it("has returns true for registered adapter", () => {
    const registry = new AdapterRegistry();
    registry.register(createMockAdapter("telegram"));
    assert.equal(registry.has("telegram"), true);
  });

  it("has returns false for unregistered adapter", () => {
    const registry = new AdapterRegistry();
    assert.equal(registry.has("telegram"), false);
  });

  it("list returns all registered adapters with info", () => {
    const registry = new AdapterRegistry();
    registry.register(createMockAdapter("telegram", "bidirectional", "connected"));
    registry.register(createMockAdapter("discord", "bidirectional", "disconnected"));
    registry.register(createMockAdapter("email", "outbound", "disconnected"));

    const list = registry.list();
    assert.equal(list.length, 3);

    const telegram = list.find((c) => c.name === "telegram");
    assert.ok(telegram);
    assert.equal(telegram.status, "connected");
    assert.equal(telegram.direction, "bidirectional");

    const discord = list.find((c) => c.name === "discord");
    assert.ok(discord);
    assert.equal(discord.status, "disconnected");

    const email = list.find((c) => c.name === "email");
    assert.ok(email);
    assert.equal(email.direction, "outbound");
  });

  it("register overwrites existing adapter", () => {
    const registry = new AdapterRegistry();
    registry.register(createMockAdapter("telegram", "bidirectional", "disconnected"));
    registry.register(createMockAdapter("telegram", "bidirectional", "connected"));

    const list = registry.list();
    assert.equal(list.length, 1);
    assert.equal(list[0]!.status, "connected");
  });

  it("list returns empty array when no adapters registered", () => {
    const registry = new AdapterRegistry();
    const list = registry.list();
    assert.deepEqual(list, []);
  });
});
