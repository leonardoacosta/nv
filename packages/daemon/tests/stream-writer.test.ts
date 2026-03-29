/**
 * Tests for TelegramStreamWriter.buildDisplayText via onToolDone/onToolStart.
 *
 * Covers:
 *  4.1 - verify buildDisplayText output with completed tools and elapsed timer
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";

// ── Minimal adapter stub ──────────────────────────────────────────────────────

function makeAdapter() {
  return {
    sendDraft: async (_chatId: string, _draftId: number, _text: string) => true,
    sendMessage: async (_chatId: string, _text: string, _opts?: object) =>
      ({ message_id: 1 }),
    editMessage: async (_chatId: string, _msgId: number, _text: string) => {},
    deleteMessage: async (_chatId: string, _msgId: number) => {},
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Access buildDisplayText without triggering flush (we call it via onToolStart/onToolDone) */
function getBuildDisplayText(writer: unknown): () => string {
  // buildDisplayText is a private method — access it for testing via bracket notation
  return () => (writer as Record<string, unknown>)["buildDisplayText"]?.call(writer) as string ?? "";
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("TelegramStreamWriter.buildDisplayText", () => {
  it("returns empty string when no tools have started and no text", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    const text = getBuildDisplayText(writer)();
    assert.equal(text, "", "empty writer must produce empty display text");
  });

  it("shows active tool with live elapsed timer", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    writer.onToolStart("read_memory", "call-1");

    // Give a tiny bit of time for elapsed to be > 0
    await new Promise((resolve) => setTimeout(resolve, 50));

    const text = getBuildDisplayText(writer)();

    // Must contain the humanized tool name
    assert.ok(text.length > 0, "display text must not be empty with active tool");
    assert.ok(
      text.includes("(") && text.includes("s)"),
      `active tool display must include elapsed seconds, got: ${text}`,
    );
    // Must include total elapsed marker
    assert.ok(
      text.includes("— ") && text.includes("s total"),
      `display must include total elapsed, got: ${text}`,
    );
  });

  it("shows completed tool with real duration (not live timer)", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    // Start a tool and immediately complete it with a known duration
    writer.onToolStart("write_memory", "call-2");
    writer.onToolDone("write_memory", "call-2", 2500); // 2.5 seconds → rounds to 3s

    const text = getBuildDisplayText(writer)();

    assert.ok(text.length > 0, "display text must not be empty after tool completes");
    // Completed tool shows its actual duration (2.5s → 3s rounded)
    assert.ok(
      text.includes("(3s)") || text.includes("(2s)"),
      `completed tool must show real duration, got: ${text}`,
    );
    // No active tools remain — but total elapsed timer must still appear
    assert.ok(
      text.includes("s total"),
      `display must include total elapsed, got: ${text}`,
    );
    // Active tool section must be gone
    assert.ok(
      !text.includes("undefined"),
      `display must not contain 'undefined', got: ${text}`,
    );
  });

  it("shows completed tools and active tools together", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    // Complete one tool
    writer.onToolStart("read_memory", "call-1");
    writer.onToolDone("read_memory", "call-1", 1200); // 1.2s → 1s

    // Start another tool (still active)
    writer.onToolStart("write_memory", "call-2");

    const text = getBuildDisplayText(writer)();

    // Both tools must appear in the display
    assert.ok(text.length > 0, "display text must not be empty");
    assert.ok(text.includes("|"), "multiple tools must be separated by '|'");
    assert.ok(text.includes("s total"), "total elapsed must appear");
  });

  it("retains only last 3 completed tools (cap)", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    // Complete 4 tools — only the last 3 should appear
    writer.onToolStart("tool_a", "c1");
    writer.onToolDone("tool_a", "c1", 1000);

    writer.onToolStart("tool_b", "c2");
    writer.onToolDone("tool_b", "c2", 2000);

    writer.onToolStart("tool_c", "c3");
    writer.onToolDone("tool_c", "c3", 3000);

    writer.onToolStart("tool_d", "c4");
    writer.onToolDone("tool_d", "c4", 4000);

    // Access the private completedTools array to verify capping
    const completedTools = (writer as unknown as Record<string, unknown>)["completedTools"] as Array<{
      humanized: string;
      durationMs: number;
    }>;

    assert.equal(completedTools.length, 3, "completedTools must be capped at 3");
    // First item (tool_a) must have been evicted
    const names = completedTools.map((t) => t.humanized);
    assert.ok(!names.some((n) => n.toLowerCase().includes("tool_a")), "oldest tool must be evicted");
  });

  it("includes accumulated text in display below tool status line", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    writer.onTextDelta("Hello, world!");
    writer.onToolStart("read_memory", "call-1");
    writer.onToolDone("read_memory", "call-1", 500);

    const text = getBuildDisplayText(writer)();

    assert.ok(text.includes("Hello, world!"), `text delta must appear in display, got: ${text}`);
    assert.ok(text.includes("s total"), `tool timing must appear in display, got: ${text}`);
  });

  it("firstEventAt is set on first tool start and drives total elapsed", async () => {
    const { TelegramStreamWriter } = await import("../src/channels/stream-writer.js");
    const adapter = makeAdapter();
    const writer = new TelegramStreamWriter(adapter as never, "chat123");

    // firstEventAt must be null before any events
    const firstEventBefore = (writer as unknown as Record<string, unknown>)["firstEventAt"] as number | null;
    assert.equal(firstEventBefore, null, "firstEventAt must be null before any events");

    writer.onToolStart("read_memory", "call-1");

    const firstEventAfter = (writer as unknown as Record<string, unknown>)["firstEventAt"] as number | null;
    assert.ok(firstEventAfter !== null, "firstEventAt must be set after first tool start");
    assert.ok(firstEventAfter! > 0, "firstEventAt must be a positive timestamp");

    // Complete the tool and check display
    writer.onToolDone("read_memory", "call-1", 1000);
    const text = getBuildDisplayText(writer)();
    assert.ok(text.includes("s total"), "total elapsed must be derived from firstEventAt");
  });
});
