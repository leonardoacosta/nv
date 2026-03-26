import { describe, it, afterEach } from "node:test";
import assert from "node:assert/strict";

import { DiscordClient } from "../src/client.js";
import { listChannels } from "../src/tools/channels.js";
import { readMessages } from "../src/tools/messages.js";
import { listGuilds } from "../src/tools/guilds.js";

// Helper to mock fetch and create a client
function createMockClient(responseData: unknown): {
  client: DiscordClient;
  restore: () => void;
} {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () =>
    new Response(JSON.stringify(responseData), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }) as Response;

  return {
    client: new DiscordClient("test-token"),
    restore: () => {
      globalThis.fetch = originalFetch;
    },
  };
}

describe("listGuilds", () => {
  let restore: () => void;

  afterEach(() => {
    if (restore) restore();
  });

  it("returns guilds with id, name, icon", async () => {
    const mock = createMockClient([
      { id: "1", name: "Test Guild", icon: "abc123" },
      { id: "2", name: "Another Guild", icon: null },
    ]);
    restore = mock.restore;

    const result = await listGuilds(mock.client);
    assert.equal(result.guilds.length, 2);
    assert.equal(result.guilds[0].id, "1");
    assert.equal(result.guilds[0].name, "Test Guild");
    assert.equal(result.guilds[0].icon, "abc123");
    assert.equal(result.guilds[1].icon, null);
  });

  it("returns empty array when bot is in no guilds", async () => {
    const mock = createMockClient([]);
    restore = mock.restore;

    const result = await listGuilds(mock.client);
    assert.deepEqual(result.guilds, []);
  });
});

describe("listChannels", () => {
  let restore: () => void;

  afterEach(() => {
    if (restore) restore();
  });

  it("filters to text channels only (type 0)", async () => {
    const mock = createMockClient([
      { id: "1", name: "general", type: 0, position: 0, parent_id: null },
      { id: "2", name: "voice-chat", type: 2, position: 1, parent_id: null },
      { id: "3", name: "announcements", type: 5, position: 2, parent_id: null },
    ]);
    restore = mock.restore;

    const result = await listChannels(mock.client, "guild-1");
    assert.equal(result.channels.length, 1);
    assert.equal(result.channels[0].name, "general");
  });

  it("resolves category names from type 4 channels", async () => {
    const mock = createMockClient([
      { id: "cat-1", name: "Text Channels", type: 4, position: 0, parent_id: null },
      { id: "1", name: "general", type: 0, position: 0, parent_id: "cat-1" },
      { id: "2", name: "random", type: 0, position: 1, parent_id: "cat-1" },
    ]);
    restore = mock.restore;

    const result = await listChannels(mock.client, "guild-1");
    assert.equal(result.channels.length, 2);
    assert.equal(result.channels[0].category, "Text Channels");
    assert.equal(result.channels[1].category, "Text Channels");
  });

  it("groups by category and sorts by position", async () => {
    const mock = createMockClient([
      { id: "cat-1", name: "Info", type: 4, position: 0, parent_id: null },
      { id: "cat-2", name: "Chat", type: 4, position: 1, parent_id: null },
      { id: "1", name: "rules", type: 0, position: 2, parent_id: "cat-1" },
      { id: "2", name: "faq", type: 0, position: 0, parent_id: "cat-1" },
      { id: "3", name: "general", type: 0, position: 0, parent_id: "cat-2" },
      { id: "4", name: "off-topic", type: 0, position: 1, parent_id: "cat-2" },
    ]);
    restore = mock.restore;

    const result = await listChannels(mock.client, "guild-1");
    assert.equal(result.guild_id, "guild-1");
    assert.equal(result.channels.length, 4);

    // Info category channels should be sorted by position
    const infoChannels = result.channels.filter((c) => c.category === "Info");
    assert.equal(infoChannels[0].name, "faq");
    assert.equal(infoChannels[1].name, "rules");

    // Chat category channels should be sorted by position
    const chatChannels = result.channels.filter((c) => c.category === "Chat");
    assert.equal(chatChannels[0].name, "general");
    assert.equal(chatChannels[1].name, "off-topic");
  });

  it("uses (uncategorized) for channels without a parent", async () => {
    const mock = createMockClient([
      { id: "1", name: "general", type: 0, position: 0, parent_id: null },
    ]);
    restore = mock.restore;

    const result = await listChannels(mock.client, "guild-1");
    assert.equal(result.channels[0].category, "(uncategorized)");
  });
});

describe("readMessages", () => {
  let restore: () => void;

  afterEach(() => {
    if (restore) restore();
  });

  it("filters out system messages (type !== 0)", async () => {
    const mock = createMockClient([
      {
        id: "1",
        type: 0,
        content: "Hello",
        timestamp: "2026-01-01T00:00:00Z",
        author: { username: "user1", global_name: "User One" },
      },
      {
        id: "2",
        type: 7,
        content: "user joined",
        timestamp: "2026-01-01T00:01:00Z",
        author: { username: "system", global_name: null },
      },
      {
        id: "3",
        type: 0,
        content: "World",
        timestamp: "2026-01-01T00:02:00Z",
        author: { username: "user2", global_name: null },
      },
    ]);
    restore = mock.restore;

    const result = await readMessages(mock.client, "ch-1", 50);
    assert.equal(result.messages.length, 2);
    assert.equal(result.messages[0].content, "Hello");
    assert.equal(result.messages[1].content, "World");
  });

  it("truncates content at 500 chars with ... suffix", async () => {
    const longContent = "a".repeat(600);
    const mock = createMockClient([
      {
        id: "1",
        type: 0,
        content: longContent,
        timestamp: "2026-01-01T00:00:00Z",
        author: { username: "user1", global_name: "User One" },
      },
    ]);
    restore = mock.restore;

    const result = await readMessages(mock.client, "ch-1", 50);
    assert.equal(result.messages[0].content.length, 503); // 500 + "..."
    assert.ok(result.messages[0].content.endsWith("..."));
  });

  it("does not truncate content at exactly 500 chars", async () => {
    const exactContent = "b".repeat(500);
    const mock = createMockClient([
      {
        id: "1",
        type: 0,
        content: exactContent,
        timestamp: "2026-01-01T00:00:00Z",
        author: { username: "user1", global_name: null },
      },
    ]);
    restore = mock.restore;

    const result = await readMessages(mock.client, "ch-1", 50);
    assert.equal(result.messages[0].content, exactContent);
  });

  it("uses global_name with fallback to username", async () => {
    const mock = createMockClient([
      {
        id: "1",
        type: 0,
        content: "Hello",
        timestamp: "2026-01-01T00:00:00Z",
        author: { username: "user1", global_name: "Display Name" },
      },
      {
        id: "2",
        type: 0,
        content: "World",
        timestamp: "2026-01-01T00:01:00Z",
        author: { username: "fallback_user", global_name: null },
      },
    ]);
    restore = mock.restore;

    const result = await readMessages(mock.client, "ch-1", 50);
    assert.equal(result.messages[0].author, "Display Name");
    assert.equal(result.messages[1].author, "fallback_user");
  });

  it("returns empty array for channel with no messages", async () => {
    const mock = createMockClient([]);
    restore = mock.restore;

    const result = await readMessages(mock.client, "ch-1", 50);
    assert.equal(result.channel_id, "ch-1");
    assert.deepEqual(result.messages, []);
  });

  it("clamps limit to 1-100 range", async () => {
    const originalFetch = globalThis.fetch;
    let capturedUrl = "";
    globalThis.fetch = async (input: string | URL | Request) => {
      capturedUrl = typeof input === "string" ? input : input.toString();
      return new Response(JSON.stringify([]), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }) as Response;
    };

    try {
      const client = new DiscordClient("test-token");
      await readMessages(client, "ch-1", 200);
      assert.ok(capturedUrl.includes("limit=100"));

      await readMessages(client, "ch-1", -5);
      assert.ok(capturedUrl.includes("limit=1"));
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
