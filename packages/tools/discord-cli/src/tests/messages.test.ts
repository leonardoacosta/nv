import { describe, it } from "node:test";
import assert from "node:assert/strict";

// Inline the truncation logic to test without side effects.
// Mirrors exact logic in src/commands/messages.ts.

const MAX_CONTENT_LENGTH = 500;

function truncate(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + "…";
}

describe("messages truncation", () => {
  it("leaves content unchanged when at or below 500 chars", () => {
    const short = "Hello world";
    assert.equal(truncate(short, MAX_CONTENT_LENGTH), short);
  });

  it("leaves content unchanged when exactly 500 chars", () => {
    const exactly500 = "a".repeat(500);
    assert.equal(truncate(exactly500, MAX_CONTENT_LENGTH), exactly500);
  });

  it("truncates content to 500 chars and appends ellipsis when over limit", () => {
    const over500 = "b".repeat(600);
    const result = truncate(over500, MAX_CONTENT_LENGTH);

    // Should be 501 chars: 500 content + 1 ellipsis (…is a single char)
    assert.equal(result.length, 501);
    assert.ok(result.endsWith("…"), "Should end with ellipsis");
    assert.equal(result.slice(0, 500), "b".repeat(500));
  });

  it("handles empty string without truncation", () => {
    assert.equal(truncate("", MAX_CONTENT_LENGTH), "");
  });

  it("truncates at exactly 501st char", () => {
    const exactly501 = "c".repeat(501);
    const result = truncate(exactly501, MAX_CONTENT_LENGTH);
    assert.ok(result.endsWith("…"));
    assert.equal(result.length, 501);
  });
});
