import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { MessageRouter, formatToolResponse } from "../src/brain/router.js";
import { KeywordRouter } from "../src/brain/keyword-router.js";
import type { EmbeddingRouter } from "../src/brain/embedding-router.js";

// ── Mock EmbeddingRouter ──────────────────────────────────────────────────────

function createMockEmbeddingRouter(
  matchFn: (text: string) => { tool: string; port: number; confidence: number } | null,
): EmbeddingRouter {
  return {
    match: async (text: string) => matchFn(text),
  } as unknown as EmbeddingRouter;
}

describe("MessageRouter", () => {
  describe("route()", () => {
    it("returns Tier 0 for messages starting with /", async () => {
      const router = new MessageRouter(new KeywordRouter(), null);
      const result = await router.route("/start");
      assert.equal(result.tier, 0);
      assert.equal(result.confidence, 1.0);
      assert.equal(result.tool, undefined);
    });

    it("returns Tier 0 for /help", async () => {
      const router = new MessageRouter(new KeywordRouter(), null);
      const result = await router.route("/help");
      assert.equal(result.tier, 0);
    });

    it("returns Tier 1 for keyword match", async () => {
      const router = new MessageRouter(new KeywordRouter(), null);
      const result = await router.route("what's on my calendar");
      assert.equal(result.tier, 1);
      assert.equal(result.tool, "calendar_today");
      assert.equal(result.confidence, 0.95);
    });

    it("returns Tier 1 for email match", async () => {
      const router = new MessageRouter(new KeywordRouter(), null);
      const result = await router.route("check my email");
      assert.equal(result.tier, 1);
      assert.equal(result.tool, "email_inbox");
    });

    it("returns Tier 2 when keyword misses but embedding matches", async () => {
      const mockEmb = createMockEmbeddingRouter((text) => {
        if (text.includes("schedule")) {
          return { tool: "calendar_today", port: 4106, confidence: 0.88 };
        }
        return null;
      });
      const router = new MessageRouter(new KeywordRouter(), mockEmb);

      // "show me my schedule" would match keyword, so use a phrase that doesn't
      const result = await router.route("please display schedule overview");
      assert.equal(result.tier, 2);
      assert.equal(result.tool, "calendar_today");
      assert.ok(result.confidence >= 0.82);
    });

    it("returns Tier 3 when no tier matches", async () => {
      const mockEmb = createMockEmbeddingRouter(() => null);
      const router = new MessageRouter(new KeywordRouter(), mockEmb);
      const result = await router.route("tell me about quantum physics");
      assert.equal(result.tier, 3);
      assert.equal(result.confidence, 0.0);
      assert.equal(result.tool, undefined);
    });

    it("returns Tier 3 when embedding router is null (disabled)", async () => {
      const router = new MessageRouter(new KeywordRouter(), null);
      const result = await router.route("explain the theory of relativity");
      assert.equal(result.tier, 3);
      assert.equal(result.confidence, 0.0);
    });

    it("prefers Tier 1 over Tier 2", async () => {
      const mockEmb = createMockEmbeddingRouter(() => ({
        tool: "calendar_today",
        port: 4106,
        confidence: 0.95,
      }));
      const router = new MessageRouter(new KeywordRouter(), mockEmb);
      const result = await router.route("what's on my calendar");
      // Should be Tier 1 (keyword) not Tier 2 (embedding)
      assert.equal(result.tier, 1);
    });
  });
});

describe("formatToolResponse", () => {
  it("returns string result directly", () => {
    assert.equal(formatToolResponse("hello"), "hello");
  });

  it("returns text field from object", () => {
    assert.equal(formatToolResponse({ text: "formatted output" }), "formatted output");
  });

  it("unwraps result field", () => {
    assert.equal(formatToolResponse({ result: "inner value" }), "inner value");
  });

  it("formats error field", () => {
    assert.equal(formatToolResponse({ error: "not found" }), "Error: not found");
  });

  it("formats arrays as bulleted list", () => {
    const result = formatToolResponse(["item 1", "item 2", "item 3"]);
    assert.ok(result.includes("- item 1"));
    assert.ok(result.includes("- item 2"));
    assert.ok(result.includes("- item 3"));
  });

  it("formats array of objects with title/time", () => {
    const result = formatToolResponse([
      { title: "Meeting", time: "10:00 AM" },
      { title: "Lunch", time: "12:00 PM" },
    ]);
    assert.ok(result.includes("Meeting"));
    assert.ok(result.includes("10:00 AM"));
    assert.ok(result.includes("Lunch"));
  });

  it("returns 'No items found.' for empty array", () => {
    assert.equal(formatToolResponse([]), "No items found.");
  });

  it("returns 'No data returned.' for null", () => {
    assert.equal(formatToolResponse(null), "No data returned.");
  });

  it("returns 'No data returned.' for undefined", () => {
    assert.equal(formatToolResponse(undefined), "No data returned.");
  });

  it("falls back to JSON for complex objects", () => {
    const result = formatToolResponse({ nested: { deep: true }, count: 42 });
    assert.ok(result.includes("nested"));
    assert.ok(result.includes("count"));
  });
});
