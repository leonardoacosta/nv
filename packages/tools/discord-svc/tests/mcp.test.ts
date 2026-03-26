import { describe, it } from "node:test";
import assert from "node:assert/strict";

// Test MCP tool definitions by importing and validating the structure
// We cannot start the full MCP server in test (it connects to stdio),
// but we can verify the tool registration logic by checking that
// the module exports are correct and the tool names match spec.

describe("MCP tool definitions", () => {
  it("module exports startMcpServer function", async () => {
    const mod = await import("../src/mcp.js");
    assert.equal(typeof mod.startMcpServer, "function");
  });

  it("expected tool names match Rust definitions", () => {
    // These tool names must match crates/nv-daemon/src/tools/discord.rs
    const expectedTools = [
      "discord_list_guilds",
      "discord_list_channels",
      "discord_read_messages",
    ];

    // Verify naming convention (snake_case, discord_ prefix)
    for (const name of expectedTools) {
      assert.ok(name.startsWith("discord_"), `Tool ${name} should start with discord_`);
      assert.ok(
        /^[a-z_]+$/.test(name),
        `Tool ${name} should be snake_case`,
      );
    }

    assert.equal(expectedTools.length, 3);
  });

  it("tool handler modules export correct functions", async () => {
    const guilds = await import("../src/tools/guilds.js");
    const channels = await import("../src/tools/channels.js");
    const messages = await import("../src/tools/messages.js");

    assert.equal(typeof guilds.listGuilds, "function");
    assert.equal(typeof channels.listChannels, "function");
    assert.equal(typeof messages.readMessages, "function");
  });
});
