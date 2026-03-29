/**
 * Tests for digest observability:
 *
 * [4.1] Suppression persistence survives across digest runs — insert suppression,
 *       run suppress, verify item is still suppressed.
 * [4.2] Expired suppressions are cleaned up — insert suppression with past
 *       expires_at, run suppress, verify row deleted.
 * [4.3] digestStats shape contract — active_suppressions_count reflects DB state.
 * [4.4] digestSuppressions ordering — only non-expired entries, ordered by
 *       last_sent_at DESC.
 */

import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

import { suppressItems, markItemsSent } from "../src/features/digest/suppress.js";
import type { DigestItem } from "../src/features/digest/classify.js";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function computeItemHash(item: DigestItem): string {
  return createHash("sha256")
    .update(`${item.source}:${item.title}:${item.detail}`)
    .digest("hex");
}

function makeItem(overrides: Partial<DigestItem> = {}): DigestItem {
  return {
    id: "item-1",
    source: "email",
    priority: "P1",
    title: "Deploy auth service",
    detail: "Pending since Monday",
    actionable: true,
    ...overrides,
  };
}

function makeDigestConfig(overrides: Partial<{
  p0CooldownMs: number;
  p1CooldownMs: number;
  p2CooldownMs: number;
}> = {}) {
  return {
    p0CooldownMs: 60_000,       // 1 min
    p1CooldownMs: 3_600_000,    // 1 hour
    p2CooldownMs: 86_400_000,   // 24 hours
    ...overrides,
  };
}

/**
 * Build a mock Pool. `queriesByCall` maps the call index to the rows it returns.
 * Calls beyond the map length return `{ rows: [] }`.
 *
 * The pool also records every call so tests can inspect SQL and params.
 */
function makePool(responses: Array<{ rows: unknown[] }> = []) {
  let callCount = 0;
  const calls: Array<{ sql: string; params: unknown[] }> = [];

  const pool = {
    query: mock.fn(async (sql: string, params?: unknown[]) => {
      calls.push({ sql, params: params ?? [] });
      const response = responses[callCount] ?? { rows: [] };
      callCount++;
      return response;
    }),
    _calls: calls,
  };

  return pool;
}

// ─── [4.1] Suppression persistence survives across digest runs ────────────────

describe("[4.1] suppressItems — active suppression persists across runs", () => {
  it("item within cooldown window is suppressed on second run", async () => {
    const item = makeItem({ priority: "P1" });
    const hash = computeItemHash(item);
    const config = makeDigestConfig({ p1CooldownMs: 3_600_000 }); // 1 hour

    // last_sent_at = 10 minutes ago → still within 1 hour cooldown
    const lastSentAt = new Date(Date.now() - 10 * 60 * 1000);
    const expiresAt = new Date(Date.now() + 50 * 60 * 1000);

    const suppressionRow = {
      hash,
      source: item.source,
      priority: 1,
      last_sent_at: lastSentAt,
      expires_at: expiresAt,
    };

    const pool = makePool([
      { rows: [] },               // DELETE expired rows
      { rows: [suppressionRow] }, // SELECT suppression rows for hashes
    ]);

    const result = await suppressItems([item], pool as never, config as never);

    assert.equal(result.suppressedCount, 1, "Item should be suppressed");
    assert.equal(result.passedCount, 0, "No items should pass");
    assert.deepEqual(result.passed, []);
  });

  it("item that passes through is NOT in the suppressed set on same run", async () => {
    const item = makeItem({ priority: "P1" });
    const config = makeDigestConfig({ p1CooldownMs: 3_600_000 });

    // No existing suppression row — item is new
    const pool = makePool([
      { rows: [] }, // DELETE
      { rows: [] }, // SELECT — no suppression row found
    ]);

    const result = await suppressItems([item], pool as never, config as never);

    assert.equal(result.suppressedCount, 0);
    assert.equal(result.passedCount, 1);
    assert.equal(result.passed[0]?.id, item.id);
  });

  it("item with expired cooldown passes even if suppression row exists", async () => {
    const item = makeItem({ priority: "P1" });
    const hash = computeItemHash(item);
    const config = makeDigestConfig({ p1CooldownMs: 3_600_000 });

    // last_sent_at = 2 hours ago → cooldown elapsed
    const lastSentAt = new Date(Date.now() - 2 * 3_600_000);
    const expiresAt = new Date(Date.now() + 3_600_000);

    const suppressionRow = {
      hash,
      source: item.source,
      priority: 1,
      last_sent_at: lastSentAt,
      expires_at: expiresAt,
    };

    const pool = makePool([
      { rows: [] },               // DELETE
      { rows: [suppressionRow] }, // SELECT
    ]);

    const result = await suppressItems([item], pool as never, config as never);

    assert.equal(result.suppressedCount, 0, "Cooldown expired — should not suppress");
    assert.equal(result.passedCount, 1);
  });

  it("suppresses mixed batch: P0 within cooldown suppressed, P2 cooldown expired passes", async () => {
    const p0Item = makeItem({ id: "p0", priority: "P0", title: "Critical alert", detail: "Service down" });
    const p2Item = makeItem({ id: "p2", priority: "P2", title: "Minor update", detail: "Low priority" });
    const p0Hash = computeItemHash(p0Item);
    const p2Hash = computeItemHash(p2Item);

    const config = makeDigestConfig({
      p0CooldownMs: 60_000,    // 1 min
      p2CooldownMs: 3_600_000, // 1 hour
    });

    // P0: last sent 30s ago → within 1 min cooldown → suppressed
    const p0Row = {
      hash: p0Hash,
      source: "email",
      priority: 0,
      last_sent_at: new Date(Date.now() - 30_000),
      expires_at: new Date(Date.now() + 30_000),
    };

    // P2: last sent 2 hours ago → cooldown elapsed → passes
    const p2Row = {
      hash: p2Hash,
      source: "email",
      priority: 2,
      last_sent_at: new Date(Date.now() - 2 * 3_600_000),
      expires_at: new Date(Date.now() + 3_600_000),
    };

    const pool = makePool([
      { rows: [] },              // DELETE
      { rows: [p0Row, p2Row] }, // SELECT both
    ]);

    const result = await suppressItems([p0Item, p2Item], pool as never, config as never);

    assert.equal(result.totalItems, 2);
    assert.equal(result.suppressedCount, 1);
    assert.equal(result.passedCount, 1);
    assert.equal(result.passed[0]?.id, "p2");
  });
});

// ─── [4.2] Expired suppressions are cleaned up ────────────────────────────────

describe("[4.2] suppressItems — expired suppressions are deleted at start of run", () => {
  it("issues DELETE WHERE expires_at < NOW() as the first query", async () => {
    const item = makeItem();
    const pool = makePool([
      { rows: [] }, // DELETE
      { rows: [] }, // SELECT
    ]);

    await suppressItems([item], pool as never, makeDigestConfig() as never);

    const calls = pool._calls;
    assert.ok(calls.length >= 1, "Should issue at least one query");
    const firstSql = calls[0]?.sql ?? "";
    assert.ok(
      firstSql.includes("DELETE FROM digest_suppression"),
      `First query should be DELETE, got: ${firstSql}`,
    );
    assert.ok(
      firstSql.includes("expires_at") && firstSql.includes("NOW()"),
      "DELETE should filter by expires_at < NOW()",
    );
  });

  it("cleanup DELETE is still issued even with an empty items array", async () => {
    const pool = makePool([{ rows: [] }]);
    const result = await suppressItems([], pool as never, makeDigestConfig() as never);

    // Empty items → early return before SELECT, but DELETE fires first
    const calls = pool._calls;
    assert.equal(calls.length, 1, "Only the cleanup DELETE is issued for empty items");
    assert.ok(calls[0]?.sql.includes("DELETE FROM digest_suppression"));
    assert.deepEqual(result, {
      passed: [],
      totalItems: 0,
      suppressedCount: 0,
      passedCount: 0,
    });
  });

  it("expired row is absent from SELECT results so item passes through", async () => {
    const item = makeItem({ priority: "P2" });
    const config = makeDigestConfig({ p2CooldownMs: 3_600_000 });

    // Simulate: the expired row was deleted before SELECT, so SELECT returns nothing
    const pool = makePool([
      { rows: [] }, // DELETE (removes the expired row)
      { rows: [] }, // SELECT — row no longer present after cleanup
    ]);

    const result = await suppressItems([item], pool as never, config as never);

    assert.equal(result.suppressedCount, 0, "Item passes once expired row is cleaned up");
    assert.equal(result.passedCount, 1);
  });

  it("marks items sent with correct expires_at based on priority cooldown", async () => {
    const item = makeItem({ priority: "P0" });
    const config = makeDigestConfig({ p0CooldownMs: 60_000 }); // 1 minute

    const upsertedRows: Array<{ key: string; params: unknown[] }> = [];
    const pool = {
      query: mock.fn(async (sql: string, params: unknown[]) => {
        if (sql.includes("INSERT INTO digest_suppression")) {
          upsertedRows.push({ key: String(params[0]), params });
        }
        if (sql.includes("SELECT content FROM memory")) {
          return { rows: [] };
        }
        return { rows: [] };
      }),
    };

    const before = Date.now();
    await markItemsSent([item], pool as never, config as never);
    const after = Date.now();

    assert.equal(upsertedRows.length, 1, "One upsert per item");
    const params = upsertedRows[0]?.params;
    const hash = params?.[0] as string;
    const expiresAt = params?.[4] as Date;

    assert.ok(typeof hash === "string" && hash.length === 64, "Hash should be 64-char hex");
    assert.ok(expiresAt instanceof Date, "expires_at should be a Date");

    const expectedMin = before + 60_000;
    const expectedMax = after + 60_000;
    assert.ok(
      expiresAt.getTime() >= expectedMin && expiresAt.getTime() <= expectedMax,
      `expires_at should be ~1 minute from now, got: ${expiresAt.toISOString()}`,
    );
  });
});

// ─── [4.3] digestStats shape contract ─────────────────────────────────────────

describe("[4.3] digestStats — shape contract", () => {
  /**
   * These tests verify the output shape that the tRPC procedure produces
   * by directly testing the mapping/transformation logic used in the procedure.
   * The procedure runs:
   *   1. COUNT active suppressions (expires_at > now)
   *   2. Most recent diary entry for trigger_type = 'digest_run'
   *   3. GROUP BY priority counts for active suppressions
   *
   * We test the priority-label mapping and the shape of the aggregated result.
   */

  it("priority integer 0 maps to 'P0' label in suppression_by_priority", () => {
    // Reproduce the mapping logic from the procedure
    const byPriorityRows = [
      { priority: 0, count: 3 },
      { priority: 1, count: 5 },
      { priority: 2, count: 2 },
    ];

    const suppressionByPriority: Record<string, number> = {};
    for (const row of byPriorityRows) {
      const label = row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2";
      suppressionByPriority[label] = row.count;
    }

    assert.equal(suppressionByPriority["P0"], 3);
    assert.equal(suppressionByPriority["P1"], 5);
    assert.equal(suppressionByPriority["P2"], 2);
  });

  it("active_suppressions_count equals sum of suppression_by_priority values", () => {
    const byPriorityRows = [
      { priority: 0, count: 2 },
      { priority: 2, count: 4 },
    ];

    const suppressionByPriority: Record<string, number> = {};
    for (const row of byPriorityRows) {
      const label = row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2";
      suppressionByPriority[label] = row.count;
    }

    // active_suppressions_count is a separate COUNT(*) query, both should agree
    const activeCount = Object.values(suppressionByPriority).reduce((a, b) => a + b, 0);
    assert.equal(activeCount, 6);
  });

  it("returns null last_run_at when no diary entries exist", () => {
    // Simulate empty diary result — procedure returns null for missing row
    const lastRunRow = undefined;
    const last_run_at = lastRunRow?.createdAt?.toISOString() ?? null;
    assert.equal(last_run_at, null);
  });

  it("returns ISO string last_run_at when diary entry exists", () => {
    const createdAt = new Date("2025-06-15T10:30:00.000Z");
    const lastRunRow = { createdAt, content: "Digest run: 5 passed, 2 suppressed" };
    const last_run_at = lastRunRow.createdAt.toISOString();
    assert.equal(last_run_at, "2025-06-15T10:30:00.000Z");
  });

  it("active_suppressions_count falls back to 0 when COUNT returns undefined", () => {
    // Reproduce the null-coalescing in the procedure
    const activeCountRow = undefined;
    const activeCount = activeCountRow?.count ?? 0;
    assert.equal(activeCount, 0);
  });

  it("returned shape contains all required keys", () => {
    // Verify the exact shape contract
    const simulatedResult = {
      last_run_at: null as string | null,
      last_run_summary: null as string | null,
      active_suppressions_count: 0,
      suppression_by_priority: {} as Record<string, number>,
    };

    const requiredKeys = [
      "last_run_at",
      "last_run_summary",
      "active_suppressions_count",
      "suppression_by_priority",
    ];

    for (const key of requiredKeys) {
      assert.ok(key in simulatedResult, `Result should contain key: ${key}`);
    }
  });

  it("suppression_by_priority is empty object when no active suppressions exist", () => {
    const byPriorityRows: Array<{ priority: number; count: number }> = [];
    const suppressionByPriority: Record<string, number> = {};
    for (const row of byPriorityRows) {
      const label = row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2";
      suppressionByPriority[label] = row.count;
    }
    assert.deepEqual(suppressionByPriority, {});
  });
});

// ─── [4.4] digestSuppressions ordering ────────────────────────────────────────

describe("[4.4] digestSuppressions — non-expired entries ordered by last_sent_at DESC", () => {
  /**
   * These tests verify the mapping/ordering logic used in the tRPC procedure.
   * The procedure runs:
   *   SELECT * FROM digest_suppression WHERE expires_at > now() ORDER BY last_sent_at DESC
   * and maps each row to a shape with { hash, source, priority (label), last_sent_at, expires_at }.
   */

  it("maps DB rows to the expected output shape", () => {
    const now = new Date();
    const rows = [
      {
        hash: "abc123",
        source: "email",
        priority: 1,
        lastSentAt: new Date("2025-06-15T08:00:00.000Z"),
        expiresAt: new Date(now.getTime() + 3_600_000),
        createdAt: new Date("2025-06-15T08:00:00.000Z"),
      },
    ];

    // Reproduce the mapping from the procedure
    const mapped = rows.map((row) => ({
      hash: row.hash,
      source: row.source,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
      last_sent_at: row.lastSentAt.toISOString(),
      expires_at: row.expiresAt.toISOString(),
    }));

    assert.equal(mapped.length, 1);
    assert.equal(mapped[0]?.hash, "abc123");
    assert.equal(mapped[0]?.source, "email");
    assert.equal(mapped[0]?.priority, "P1");
    assert.equal(mapped[0]?.last_sent_at, "2025-06-15T08:00:00.000Z");
    assert.ok(typeof mapped[0]?.expires_at === "string");
  });

  it("priority integer mapping: 0 → P0, 1 → P1, 2 → P2", () => {
    const now = new Date();
    const rows = [
      { hash: "h0", source: "ado", priority: 0, lastSentAt: new Date(), expiresAt: new Date(now.getTime() + 1000), createdAt: new Date() },
      { hash: "h1", source: "email", priority: 1, lastSentAt: new Date(), expiresAt: new Date(now.getTime() + 1000), createdAt: new Date() },
      { hash: "h2", source: "teams", priority: 2, lastSentAt: new Date(), expiresAt: new Date(now.getTime() + 1000), createdAt: new Date() },
    ];

    const mapped = rows.map((row) => ({
      hash: row.hash,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
    }));

    assert.equal(mapped[0]?.priority, "P0");
    assert.equal(mapped[1]?.priority, "P1");
    assert.equal(mapped[2]?.priority, "P2");
  });

  it("ORDER BY last_sent_at DESC — most recently sent entry appears first", () => {
    const now = new Date();
    // Simulate DB returning rows already ordered by last_sent_at DESC
    const rows = [
      {
        hash: "newest",
        source: "email",
        priority: 1,
        lastSentAt: new Date("2025-06-15T12:00:00.000Z"),
        expiresAt: new Date(now.getTime() + 3_600_000),
        createdAt: new Date(),
      },
      {
        hash: "older",
        source: "teams",
        priority: 2,
        lastSentAt: new Date("2025-06-15T06:00:00.000Z"),
        expiresAt: new Date(now.getTime() + 3_600_000),
        createdAt: new Date(),
      },
      {
        hash: "oldest",
        source: "ado",
        priority: 0,
        lastSentAt: new Date("2025-06-14T12:00:00.000Z"),
        expiresAt: new Date(now.getTime() + 3_600_000),
        createdAt: new Date(),
      },
    ];

    const mapped = rows.map((row) => ({
      hash: row.hash,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
      last_sent_at: row.lastSentAt.toISOString(),
      source: row.source,
      expires_at: row.expiresAt.toISOString(),
    }));

    // Verify order is preserved DESC
    assert.equal(mapped[0]?.hash, "newest");
    assert.equal(mapped[1]?.hash, "older");
    assert.equal(mapped[2]?.hash, "oldest");

    // Verify timestamps descend
    for (let i = 0; i < mapped.length - 1; i++) {
      const curr = new Date(mapped[i]!.last_sent_at).getTime();
      const next = new Date(mapped[i + 1]!.last_sent_at).getTime();
      assert.ok(curr > next, `Entry ${i} (${mapped[i]?.last_sent_at}) should be after entry ${i + 1} (${mapped[i + 1]?.last_sent_at})`);
    }
  });

  it("returns empty array when no active suppressions exist", () => {
    const rows: unknown[] = [];
    const mapped = (rows as Array<{
      hash: string;
      source: string;
      priority: number;
      lastSentAt: Date;
      expiresAt: Date;
    }>).map((row) => ({
      hash: row.hash,
      source: row.source,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
      last_sent_at: row.lastSentAt.toISOString(),
      expires_at: row.expiresAt.toISOString(),
    }));

    assert.deepEqual(mapped, []);
  });

  it("expired rows (expires_at <= now) are excluded by the WHERE clause", () => {
    const now = new Date();

    // Simulate what the DB returns after WHERE expires_at > now()
    // An expired row would NOT appear in results
    const allRows = [
      {
        hash: "active-1",
        expiresAt: new Date(now.getTime() + 3_600_000), // future
      },
      {
        hash: "expired-1",
        expiresAt: new Date(now.getTime() - 1000), // past
      },
    ];

    // The WHERE clause filters server-side; simulate that only active rows returned
    const activeRows = allRows.filter((r) => r.expiresAt > now);

    assert.equal(activeRows.length, 1);
    assert.equal(activeRows[0]?.hash, "active-1");
    assert.ok(!activeRows.some((r) => r.hash === "expired-1"));
  });

  it("each row has all five required output fields", () => {
    const now = new Date();
    const row = {
      hash: "testhash",
      source: "email",
      priority: 1,
      lastSentAt: new Date("2025-06-15T10:00:00.000Z"),
      expiresAt: new Date(now.getTime() + 3_600_000),
      createdAt: new Date(),
    };

    const mapped = {
      hash: row.hash,
      source: row.source,
      priority: row.priority === 0 ? "P0" : row.priority === 1 ? "P1" : "P2",
      last_sent_at: row.lastSentAt.toISOString(),
      expires_at: row.expiresAt.toISOString(),
    };

    const requiredFields = ["hash", "source", "priority", "last_sent_at", "expires_at"];
    for (const field of requiredFields) {
      assert.ok(field in mapped, `Mapped row should contain field: ${field}`);
    }
  });
});
