/**
 * Unit tests for persistent-settings.ts
 *
 * Verifies readSettings / writeSettings / mergeOverToml round-trip so that
 * watcher config survives a simulated daemon restart (DB read → merge → apply).
 */

import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";

import {
  readSettings,
  writeSettings,
  mergeOverToml,
  PERSISTENT_KEYS,
} from "../src/features/settings/persistent-settings.js";
import type { SettingsMap } from "../src/features/settings/persistent-settings.js";

// ─── Mock Pool Factory ────────────────────────────────────────────────────────

function makePool(
  rows: Array<{ key: string; value: string }> = [],
): { query: ReturnType<typeof mock.fn> } {
  return { query: mock.fn(async () => ({ rows })) };
}

// ─── readSettings ─────────────────────────────────────────────────────────────

describe("readSettings", () => {
  it("returns an empty map when the settings table has no matching rows", async () => {
    const pool = makePool([]);
    const result = await readSettings(pool as never);
    assert.deepEqual(result, {});
  });

  it("returns a populated map when DB contains known keys", async () => {
    const pool = makePool([
      { key: "watcher.interval_minutes", value: "15" },
      { key: "watcher.enabled", value: "true" },
    ]);
    const result = await readSettings(pool as never);
    assert.equal(result["watcher.interval_minutes"], "15");
    assert.equal(result["watcher.enabled"], "true");
  });

  it("ignores keys that are not in PERSISTENT_KEYS", async () => {
    const pool = makePool([
      { key: "unknown.key", value: "should-be-ignored" },
      { key: "watcher.prompt", value: "my prompt" },
    ]);
    const result = await readSettings(pool as never);
    assert.equal(Object.keys(result).length, 1);
    assert.equal(result["watcher.prompt"], "my prompt");
  });

  it("issues the SELECT query with PERSISTENT_KEYS as the $1 parameter", async () => {
    const pool = makePool([]);
    await readSettings(pool as never);

    const calls = (pool.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(calls.length, 1);
    const sql: string = calls[0]?.arguments[0] as string;
    assert.ok(sql.includes("SELECT key, value FROM settings"), "Query should select key/value");
    const params = calls[0]?.arguments[1] as unknown[];
    assert.deepEqual(params?.[0], PERSISTENT_KEYS);
  });

  it("returns empty map (does not throw) when DB query errors", async () => {
    const pool = {
      query: mock.fn(async () => { throw new Error("DB connection refused"); }),
    };
    const result = await readSettings(pool as never);
    assert.deepEqual(result, {});
  });
});

// ─── writeSettings ────────────────────────────────────────────────────────────

describe("writeSettings", () => {
  it("upserts each provided key into the settings table", async () => {
    const pool = makePool([]);
    await writeSettings(pool as never, {
      "watcher.interval_minutes": "30",
      "watcher.enabled": "false",
    });

    const calls = (pool.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(calls.length, 2, "One upsert per key");

    // Collect the key params from both calls
    const keyParams = calls.map((c) => (c.arguments[1] as unknown[])?.[0]);
    assert.ok(keyParams.includes("watcher.interval_minutes"));
    assert.ok(keyParams.includes("watcher.enabled"));
  });

  it("each upsert uses ON CONFLICT … DO UPDATE SET", async () => {
    const pool = makePool([]);
    await writeSettings(pool as never, { "briefing.hour": "8" });

    const calls = (pool.query as ReturnType<typeof mock.fn>).mock.calls;
    const sql: string = calls[0]?.arguments[0] as string;
    assert.ok(sql.includes("ON CONFLICT"), "Should contain ON CONFLICT clause");
    assert.ok(sql.includes("DO UPDATE SET"), "Should update on conflict");
  });

  it("skips entries where value is undefined", async () => {
    const pool = makePool([]);
    // TypeScript won't allow undefined directly, so cast through unknown
    const entries: SettingsMap = {
      "watcher.enabled": "true",
    };
    // Inject undefined manually to test runtime guard
    (entries as Record<string, string | undefined>)["watcher.prompt"] = undefined;
    await writeSettings(pool as never, entries);

    const calls = (pool.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(calls.length, 1, "Only the defined key should be upserted");
    const params = calls[0]?.arguments[1] as unknown[];
    assert.equal(params?.[0], "watcher.enabled");
  });

  it("issues no queries when entries object is empty", async () => {
    const pool = makePool([]);
    await writeSettings(pool as never, {});
    const calls = (pool.query as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(calls.length, 0);
  });
});

// ─── mergeOverToml ────────────────────────────────────────────────────────────

describe("mergeOverToml", () => {
  it("returns empty overrides and empty overriddenKeys when DB map is empty", () => {
    const result = mergeOverToml({});
    assert.deepEqual(result.watcher, {});
    assert.deepEqual(result.briefing, {});
    assert.deepEqual(result.overriddenKeys, []);
  });

  it("parses interval_minutes as a number and records it in overriddenKeys", () => {
    const result = mergeOverToml({ "watcher.interval_minutes": "15" });
    assert.equal(result.watcher.intervalMinutes, 15);
    assert.ok(result.overriddenKeys.includes("watcher.interval_minutes"));
  });

  it("parses watcher.enabled='true' as boolean true", () => {
    const result = mergeOverToml({ "watcher.enabled": "true" });
    assert.equal(result.watcher.enabled, true);
    assert.ok(result.overriddenKeys.includes("watcher.enabled"));
  });

  it("parses watcher.enabled='false' as boolean false", () => {
    const result = mergeOverToml({ "watcher.enabled": "false" });
    assert.equal(result.watcher.enabled, false);
  });

  it("parses briefing.hour as a number", () => {
    const result = mergeOverToml({ "briefing.hour": "9" });
    assert.equal(result.briefing.hour, 9);
    assert.ok(result.overriddenKeys.includes("briefing.hour"));
  });

  it("ignores interval_minutes when value is non-numeric", () => {
    const result = mergeOverToml({ "watcher.interval_minutes": "not-a-number" });
    assert.equal(result.watcher.intervalMinutes, undefined);
    assert.ok(!result.overriddenKeys.includes("watcher.interval_minutes"));
  });

  it("passes through quiet_start and quiet_end as strings", () => {
    const result = mergeOverToml({
      "watcher.quiet_start": "22:00",
      "watcher.quiet_end": "07:00",
    });
    assert.equal(result.watcher.quietStart, "22:00");
    assert.equal(result.watcher.quietEnd, "07:00");
    assert.ok(result.overriddenKeys.includes("watcher.quiet_start"));
    assert.ok(result.overriddenKeys.includes("watcher.quiet_end"));
  });

  it("passes through watcher.prompt as a string", () => {
    const result = mergeOverToml({ "watcher.prompt": "custom prompt text" });
    assert.equal(result.watcher.prompt, "custom prompt text");
    assert.ok(result.overriddenKeys.includes("watcher.prompt"));
  });

  it("passes through briefing.prompt as a string", () => {
    const result = mergeOverToml({ "briefing.prompt": "morning briefing prompt" });
    assert.equal(result.briefing.prompt, "morning briefing prompt");
    assert.ok(result.overriddenKeys.includes("briefing.prompt"));
  });
});

// ─── Round-trip: writeSettings → readSettings → mergeOverToml ────────────────

describe("round-trip: write → read → merge", () => {
  it("interval_minutes written and read back produces the correct numeric override", async () => {
    // Simulate writing interval_minutes=20 to DB
    const writtenRows: Array<{ key: string; value: string }> = [];
    const writePool = {
      query: mock.fn(async (_sql: string, params: unknown[]) => {
        // Capture upserted key/value
        writtenRows.push({ key: params[0] as string, value: params[1] as string });
        return { rows: [] };
      }),
    };

    await writeSettings(writePool as never, { "watcher.interval_minutes": "20" });
    assert.equal(writtenRows.length, 1);
    assert.equal(writtenRows[0]?.key, "watcher.interval_minutes");
    assert.equal(writtenRows[0]?.value, "20");

    // Simulate daemon restart: pool now returns the rows that were written
    const readPool = makePool(writtenRows);
    const dbSettings = await readSettings(readPool as never);
    assert.equal(dbSettings["watcher.interval_minutes"], "20");

    // Merge over TOML (TOML default would have been e.g. 30)
    const merged = mergeOverToml(dbSettings);
    assert.equal(merged.watcher.intervalMinutes, 20);
    assert.ok(
      merged.overriddenKeys.includes("watcher.interval_minutes"),
      "Should report interval_minutes as overridden",
    );
  });

  it("all watcher fields written survive read → merge", async () => {
    const storedRows = [
      { key: "watcher.enabled", value: "false" },
      { key: "watcher.interval_minutes", value: "45" },
      { key: "watcher.quiet_start", value: "23:00" },
      { key: "watcher.quiet_end", value: "06:30" },
      { key: "watcher.prompt", value: "check outstanding tasks" },
    ];

    const readPool = makePool(storedRows);
    const dbSettings = await readSettings(readPool as never);
    const merged = mergeOverToml(dbSettings);

    assert.equal(merged.watcher.enabled, false);
    assert.equal(merged.watcher.intervalMinutes, 45);
    assert.equal(merged.watcher.quietStart, "23:00");
    assert.equal(merged.watcher.quietEnd, "06:30");
    assert.equal(merged.watcher.prompt, "check outstanding tasks");
    assert.equal(merged.overriddenKeys.length, 5);
  });
});
