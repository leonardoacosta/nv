import { describe, test, expect } from "vitest";
import {
  // Message schemas (vector embedding + jsonb metadata overrides)
  insertMessageSchema,
  selectMessageSchema,
  // Memory schemas (vector embedding override)
  insertMemorySchema,
  selectMemorySchema,
  // Contact schemas (jsonb channelIds override)
  insertContactSchema,
  selectContactSchema,
  // Session event schemas (jsonb metadata override)
  insertSessionEventSchema,
  selectSessionEventSchema,
  // Briefing schemas (jsonb sourcesStatus + suggestedActions overrides)
  insertBriefingSchema,
  selectBriefingSchema,
  // Diary schemas (jsonb toolsUsed override)
  insertDiarySchema,
  selectDiarySchema,
} from "../index.js";

// ─── Vector field overrides (embedding as number[]) ─────────────────────────

describe("message embedding (vector field)", () => {
  test("insertMessageSchema accepts number array for embedding", () => {
    const result = insertMessageSchema.parse({
      channel: "discord",
      content: "test",
      embedding: [0.1, 0.2, 0.3, 0.4],
    });
    expect(result.embedding).toEqual([0.1, 0.2, 0.3, 0.4]);
  });

  test("insertMessageSchema accepts undefined embedding", () => {
    const result = insertMessageSchema.parse({
      channel: "discord",
      content: "test",
    });
    expect(result.embedding).toBeUndefined();
  });

  test("insertMessageSchema rejects non-number array for embedding", () => {
    expect(() =>
      insertMessageSchema.parse({
        channel: "discord",
        content: "test",
        embedding: ["a", "b"],
      }),
    ).toThrow();
  });

  test("selectMessageSchema accepts nullable number array for embedding", () => {
    const result = selectMessageSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      channel: "discord",
      content: "test",
      createdAt: new Date(),
      embedding: [0.1, 0.2, 0.3],
      metadata: null,
      sender: null,
    });
    expect(result.embedding).toEqual([0.1, 0.2, 0.3]);
  });

  test("selectMessageSchema accepts null embedding", () => {
    const result = selectMessageSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      channel: "discord",
      content: "test",
      createdAt: new Date(),
      embedding: null,
      metadata: null,
      sender: null,
    });
    expect(result.embedding).toBeNull();
  });
});

describe("memory embedding (vector field)", () => {
  test("insertMemorySchema accepts number array for embedding", () => {
    const result = insertMemorySchema.parse({
      topic: "test-topic",
      content: "test content",
      embedding: [0.5, 0.6, 0.7],
    });
    expect(result.embedding).toEqual([0.5, 0.6, 0.7]);
  });

  test("insertMemorySchema accepts undefined embedding", () => {
    const result = insertMemorySchema.parse({
      topic: "test-topic",
      content: "test content",
    });
    expect(result.embedding).toBeUndefined();
  });

  test("selectMemorySchema accepts nullable number array", () => {
    const result = selectMemorySchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      topic: "test-topic",
      content: "test content",
      updatedAt: new Date(),
      embedding: [1.0, 2.0],
    });
    expect(result.embedding).toEqual([1.0, 2.0]);
  });

  test("selectMemorySchema accepts null embedding", () => {
    const result = selectMemorySchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      topic: "test-topic",
      content: "test content",
      updatedAt: new Date(),
      embedding: null,
    });
    expect(result.embedding).toBeNull();
  });
});

// ─── JSONB field overrides (metadata as record) ─────────────────────────────

describe("message metadata (jsonb field)", () => {
  test("insertMessageSchema accepts record for metadata", () => {
    const result = insertMessageSchema.parse({
      channel: "discord",
      content: "test",
      metadata: { source: "api", version: 2 },
    });
    expect(result.metadata).toEqual({ source: "api", version: 2 });
  });

  test("insertMessageSchema accepts null metadata", () => {
    const result = insertMessageSchema.parse({
      channel: "discord",
      content: "test",
      metadata: null,
    });
    expect(result.metadata).toBeNull();
  });

  test("selectMessageSchema accepts record for metadata", () => {
    const result = selectMessageSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      channel: "discord",
      content: "test",
      createdAt: new Date(),
      embedding: null,
      metadata: { key: "value" },
      sender: null,
    });
    expect(result.metadata).toEqual({ key: "value" });
  });

  test("selectMessageSchema accepts null metadata", () => {
    const result = selectMessageSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      channel: "discord",
      content: "test",
      createdAt: new Date(),
      embedding: null,
      metadata: null,
      sender: null,
    });
    expect(result.metadata).toBeNull();
  });
});

describe("sessionEvent metadata (jsonb field)", () => {
  test("insertSessionEventSchema accepts record for metadata", () => {
    const result = insertSessionEventSchema.parse({
      sessionId: "550e8400-e29b-41d4-a716-446655440000",
      eventType: "tool_call",
      metadata: { tool: "bash", duration: 123 },
    });
    expect(result.metadata).toEqual({ tool: "bash", duration: 123 });
  });

  test("insertSessionEventSchema accepts null metadata", () => {
    const result = insertSessionEventSchema.parse({
      sessionId: "550e8400-e29b-41d4-a716-446655440000",
      eventType: "tool_call",
      metadata: null,
    });
    expect(result.metadata).toBeNull();
  });

  test("selectSessionEventSchema accepts record for metadata", () => {
    const result = selectSessionEventSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      sessionId: "550e8400-e29b-41d4-a716-446655440000",
      eventType: "tool_call",
      createdAt: new Date(),
      direction: null,
      content: null,
      metadata: { nested: { deep: true } },
    });
    expect(result.metadata).toEqual({ nested: { deep: true } });
  });
});

// ─── JSONB field overrides (channelIds as record) ───────────────────────────

describe("contact channelIds (jsonb field)", () => {
  test("insertContactSchema accepts record for channelIds", () => {
    const result = insertContactSchema.parse({
      name: "Alice",
      channelIds: { discord: "12345", slack: "U789" },
    });
    expect(result.channelIds).toEqual({ discord: "12345", slack: "U789" });
  });

  test("insertContactSchema defaults channelIds to empty record", () => {
    const result = insertContactSchema.parse({
      name: "Alice",
    });
    expect(result.channelIds).toEqual({});
  });

  test("selectContactSchema accepts record for channelIds", () => {
    const result = selectContactSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      name: "Alice",
      channelIds: { discord: "12345" },
      createdAt: new Date(),
      relationshipType: null,
      notes: null,
    });
    expect(result.channelIds).toEqual({ discord: "12345" });
  });

  test("rejects non-record values for channelIds", () => {
    expect(() =>
      insertContactSchema.parse({
        name: "Alice",
        channelIds: "not-a-record",
      }),
    ).toThrow();
  });
});

// ─── JSONB field overrides (briefing sourcesStatus + suggestedActions) ───────

describe("briefing sourcesStatus (jsonb field)", () => {
  test("insertBriefingSchema accepts record for sourcesStatus", () => {
    const result = insertBriefingSchema.parse({
      content: "Daily briefing",
      sourcesStatus: { github: "ok", discord: "error" },
    });
    expect(result.sourcesStatus).toEqual({ github: "ok", discord: "error" });
  });

  test("insertBriefingSchema defaults sourcesStatus to empty object", () => {
    const result = insertBriefingSchema.parse({
      content: "Daily briefing",
    });
    expect(result.sourcesStatus).toEqual({});
  });

  test("selectBriefingSchema accepts record for sourcesStatus", () => {
    const result = selectBriefingSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      generatedAt: new Date(),
      content: "Daily briefing",
      sourcesStatus: { github: "ok" },
      suggestedActions: [],
    });
    expect(result.sourcesStatus).toEqual({ github: "ok" });
  });
});

describe("briefing suggestedActions (jsonb field)", () => {
  test("insertBriefingSchema accepts array for suggestedActions", () => {
    const result = insertBriefingSchema.parse({
      content: "Daily briefing",
      suggestedActions: [{ action: "review", target: "PR #42" }],
    });
    expect(result.suggestedActions).toEqual([
      { action: "review", target: "PR #42" },
    ]);
  });

  test("insertBriefingSchema defaults suggestedActions to empty array", () => {
    const result = insertBriefingSchema.parse({
      content: "Daily briefing",
    });
    expect(result.suggestedActions).toEqual([]);
  });

  test("selectBriefingSchema accepts array for suggestedActions", () => {
    const result = selectBriefingSchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      generatedAt: new Date(),
      content: "Daily briefing",
      sourcesStatus: {},
      suggestedActions: ["action1", "action2"],
    });
    expect(result.suggestedActions).toEqual(["action1", "action2"]);
  });
});

// ─── JSONB field overrides (diary toolsUsed) ────────────────────────────────

describe("diary toolsUsed (jsonb field)", () => {
  test("insertDiarySchema accepts string array for toolsUsed", () => {
    const result = insertDiarySchema.parse({
      triggerType: "cron",
      triggerSource: "scheduler",
      channel: "discord",
      slug: "daily-2024-01-01",
      content: "Today I helped with...",
      toolsUsed: ["bash", "read", "edit"],
    });
    expect(result.toolsUsed).toEqual(["bash", "read", "edit"]);
  });

  test("insertDiarySchema accepts null toolsUsed", () => {
    const result = insertDiarySchema.parse({
      triggerType: "cron",
      triggerSource: "scheduler",
      channel: "discord",
      slug: "daily-2024-01-01",
      content: "Today I helped with...",
      toolsUsed: null,
    });
    expect(result.toolsUsed).toBeNull();
  });

  test("selectDiarySchema accepts string array for toolsUsed", () => {
    const result = selectDiarySchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      triggerType: "cron",
      triggerSource: "scheduler",
      channel: "discord",
      slug: "daily-2024-01-01",
      content: "Today I helped with...",
      createdAt: new Date(),
      toolsUsed: ["bash", "read"],
      tokensIn: null,
      tokensOut: null,
      responseLatencyMs: null,
      routingTier: null,
      routingConfidence: null,
    });
    expect(result.toolsUsed).toEqual(["bash", "read"]);
  });

  test("selectDiarySchema accepts null toolsUsed", () => {
    const result = selectDiarySchema.parse({
      id: "550e8400-e29b-41d4-a716-446655440000",
      triggerType: "cron",
      triggerSource: "scheduler",
      channel: "discord",
      slug: "daily-2024-01-01",
      content: "Today I helped with...",
      createdAt: new Date(),
      toolsUsed: null,
      tokensIn: null,
      tokensOut: null,
      responseLatencyMs: null,
      routingTier: null,
      routingConfidence: null,
    });
    expect(result.toolsUsed).toBeNull();
  });

  test("insertDiarySchema rejects non-string array for toolsUsed", () => {
    expect(() =>
      insertDiarySchema.parse({
        triggerType: "cron",
        triggerSource: "scheduler",
        channel: "discord",
        slug: "daily-2024-01-01",
        content: "Today I helped with...",
        toolsUsed: [1, 2, 3],
      }),
    ).toThrow();
  });
});

// ─── Custom type fallbacks ──────────────────────────────────────────────────

describe("custom type fallback behavior", () => {
  test("vector fields fall back to z.any() without override (drizzle-zod default)", () => {
    // Without the override in our schemas, drizzle-zod would produce z.any()
    // for custom types. Our overrides ensure proper z.array(z.number()) typing.
    // This test verifies the override is working by confirming type rejection.
    expect(() =>
      insertMessageSchema.parse({
        channel: "discord",
        content: "test",
        embedding: "not-an-array",
      }),
    ).toThrow();
  });

  test("jsonb fields fall back to z.any() without override (drizzle-zod default)", () => {
    // Without override, jsonb columns would accept anything via z.any().
    // Our overrides enforce z.record() or z.array() as appropriate.
    // Contact channelIds must be a record, not a primitive.
    expect(() =>
      insertContactSchema.parse({
        name: "Alice",
        channelIds: 42,
      }),
    ).toThrow();
  });

  test("diary toolsUsed rejects non-array", () => {
    expect(() =>
      insertDiarySchema.parse({
        triggerType: "cron",
        triggerSource: "scheduler",
        channel: "discord",
        slug: "test",
        content: "test",
        toolsUsed: "not-an-array",
      }),
    ).toThrow();
  });

  test("briefing sourcesStatus rejects non-record", () => {
    expect(() =>
      insertBriefingSchema.parse({
        content: "test",
        sourcesStatus: [1, 2, 3],
      }),
    ).toThrow();
  });

  test("briefing suggestedActions rejects non-array", () => {
    expect(() =>
      insertBriefingSchema.parse({
        content: "test",
        suggestedActions: "not-an-array",
      }),
    ).toThrow();
  });
});
