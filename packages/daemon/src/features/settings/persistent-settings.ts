/**
 * Persistent settings layer for watcher and briefing configuration.
 *
 * Reads from and writes to the `settings` table using dot-notation keys
 * (e.g. "watcher.enabled", "briefing.hour"). DB values take precedence
 * over TOML defaults during startup hydration.
 */

import type { Pool } from "pg";
import { createLogger } from "../../logger.js";

const log = createLogger("persistent-settings");

// ─── Supported keys ───────────────────────────────────────────────────────────

export const PERSISTENT_KEYS = [
  "watcher.enabled",
  "watcher.interval_minutes",
  "watcher.quiet_start",
  "watcher.quiet_end",
  "watcher.prompt",
  "briefing.hour",
  "briefing.prompt",
] as const;

export type PersistentKey = (typeof PERSISTENT_KEYS)[number];

export type SettingsMap = Partial<Record<PersistentKey, string>>;

// ─── DB row type ──────────────────────────────────────────────────────────────

interface SettingRow {
  key: string;
  value: string;
}

// ─── readSettings ─────────────────────────────────────────────────────────────

/**
 * Read all persisted watcher/briefing settings from the `settings` table.
 * Returns a partial map of key -> value strings.
 * Never throws — returns empty map on DB error.
 */
export async function readSettings(pool: Pool): Promise<SettingsMap> {
  try {
    const result = await pool.query<SettingRow>(
      `SELECT key, value FROM settings WHERE key = ANY($1)`,
      [PERSISTENT_KEYS as unknown as string[]],
    );

    const map: SettingsMap = {};
    for (const row of result.rows) {
      if (PERSISTENT_KEYS.includes(row.key as PersistentKey)) {
        map[row.key as PersistentKey] = row.value;
      }
    }
    return map;
  } catch (err) {
    log.warn({ err }, "readSettings: DB query failed — returning empty map");
    return {};
  }
}

// ─── writeSettings ────────────────────────────────────────────────────────────

/**
 * Upsert one or more settings into the `settings` table.
 * Each entry is upserted independently so a partial batch is still persisted.
 */
export async function writeSettings(
  pool: Pool,
  entries: Partial<Record<PersistentKey, string>>,
): Promise<void> {
  for (const [key, value] of Object.entries(entries)) {
    if (value === undefined) continue;
    await pool.query(
      `INSERT INTO settings (key, value, updated_at)
       VALUES ($1, $2, NOW())
       ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()`,
      [key, value],
    );
  }
}

// ─── mergeOverToml ────────────────────────────────────────────────────────────

export interface WatcherOverrides {
  enabled?: boolean;
  intervalMinutes?: number;
  quietStart?: string;
  quietEnd?: string;
  prompt?: string;
}

export interface BriefingOverrides {
  hour?: number;
  prompt?: string;
}

export interface MergedSettings {
  watcher: WatcherOverrides;
  briefing: BriefingOverrides;
  overriddenKeys: string[];
}

/**
 * Merge DB settings over TOML-loaded config values.
 *
 * Returns the merged overrides and a list of which keys were actually
 * overridden from DB (used for INFO logging at startup).
 */
export function mergeOverToml(dbSettings: SettingsMap): MergedSettings {
  const overriddenKeys: string[] = [];
  const watcher: WatcherOverrides = {};
  const briefing: BriefingOverrides = {};

  if (dbSettings["watcher.enabled"] !== undefined) {
    watcher.enabled = dbSettings["watcher.enabled"] === "true";
    overriddenKeys.push("watcher.enabled");
  }
  if (dbSettings["watcher.interval_minutes"] !== undefined) {
    const parsed = parseInt(dbSettings["watcher.interval_minutes"], 10);
    if (!isNaN(parsed)) {
      watcher.intervalMinutes = parsed;
      overriddenKeys.push("watcher.interval_minutes");
    }
  }
  if (dbSettings["watcher.quiet_start"] !== undefined) {
    watcher.quietStart = dbSettings["watcher.quiet_start"];
    overriddenKeys.push("watcher.quiet_start");
  }
  if (dbSettings["watcher.quiet_end"] !== undefined) {
    watcher.quietEnd = dbSettings["watcher.quiet_end"];
    overriddenKeys.push("watcher.quiet_end");
  }
  if (dbSettings["watcher.prompt"] !== undefined) {
    watcher.prompt = dbSettings["watcher.prompt"];
    overriddenKeys.push("watcher.prompt");
  }
  if (dbSettings["briefing.hour"] !== undefined) {
    const parsed = parseInt(dbSettings["briefing.hour"], 10);
    if (!isNaN(parsed)) {
      briefing.hour = parsed;
      overriddenKeys.push("briefing.hour");
    }
  }
  if (dbSettings["briefing.prompt"] !== undefined) {
    briefing.prompt = dbSettings["briefing.prompt"];
    overriddenKeys.push("briefing.prompt");
  }

  return { watcher, briefing, overriddenKeys };
}
