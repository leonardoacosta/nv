import { createHash } from "node:crypto";
import type { Pool } from "pg";
import { createLogger } from "../../logger.js";
import type { DigestConfig } from "../../config.js";
import type { DigestItem, Priority } from "./classify.js";

const log = createLogger("digest:suppress");

// ─── Types ────────────────────────────────────────────────────────────────────

/**
 * DigestMeta no longer carries sentHashes — suppression state lives in
 * the digest_suppression table. Retained fields: lastDigestAt, weeklyStats.
 */
interface DigestMeta {
  lastDigestAt: number | null;
  weeklyStats?: WeeklyStats;
}

export interface WeeklyStats {
  itemsBySource: Record<string, number>;
  itemsByPriority: Record<string, number>;
  totalItems: number;
  digestRuns: number;
}

export interface SuppressResult {
  passed: DigestItem[];
  totalItems: number;
  suppressedCount: number;
  passedCount: number;
}

const DIGEST_META_TOPIC = "_digest_meta";

// ─── State Persistence (meta only) ───────────────────────────────────────────

interface MemoryRow {
  content: string;
}

async function readDigestMeta(pool: Pool): Promise<DigestMeta> {
  try {
    const result = await pool.query<MemoryRow>(
      `SELECT content FROM memory WHERE topic = $1 LIMIT 1`,
      [DIGEST_META_TOPIC],
    );

    if (result.rows[0]) {
      const parsed = JSON.parse(result.rows[0].content) as Partial<DigestMeta & { sentHashes: unknown }>;
      return {
        lastDigestAt: parsed.lastDigestAt ?? null,
        weeklyStats: parsed.weeklyStats,
        // sentHashes intentionally dropped — migrated to digest_suppression table
      };
    }
  } catch {
    // Malformed JSON or missing row — start fresh
  }

  return { lastDigestAt: null };
}

async function writeDigestMeta(pool: Pool, meta: DigestMeta): Promise<void> {
  const content = JSON.stringify(meta);

  await pool.query(
    `INSERT INTO memory (topic, content, updated_at)
     VALUES ($1, $2, NOW())
     ON CONFLICT (topic) DO UPDATE SET content = $2, updated_at = NOW()`,
    [DIGEST_META_TOPIC, content],
  );
}

// ─── Hash Computation ─────────────────────────────────────────────────────────

function computeItemHash(item: DigestItem): string {
  return createHash("sha256")
    .update(`${item.source}:${item.title}:${item.detail}`)
    .digest("hex");
}

// ─── Priority Numeric Mapping ─────────────────────────────────────────────────

function priorityToInt(priority: Priority): number {
  switch (priority) {
    case "P0": return 0;
    case "P1": return 1;
    case "P2": return 2;
  }
}

// ─── Cooldown Resolution ──────────────────────────────────────────────────────

function getCooldownMs(priority: Priority, config: DigestConfig): number {
  switch (priority) {
    case "P0":
      return config.p0CooldownMs;
    case "P1":
      return config.p1CooldownMs;
    case "P2":
      return config.p2CooldownMs;
  }
}

// ─── Suppression ──────────────────────────────────────────────────────────────

interface SuppressionRow {
  hash: string;
  source: string;
  priority: number;
  last_sent_at: Date;
  expires_at: Date;
}

/**
 * Suppress digest items that are within their cooldown window.
 *
 * Steps:
 * 1. Delete expired rows (expires_at < now) — cleanup pass.
 * 2. Query active suppressions for the incoming item hashes.
 * 3. For each item: if a matching suppression row exists and its cooldown
 *    has not elapsed, suppress it; otherwise pass it through.
 *
 * Returns a SuppressResult with the passed items and aggregate stats for logging.
 */
export async function suppressItems(
  items: DigestItem[],
  pool: Pool,
  config: DigestConfig,
): Promise<SuppressResult> {
  const now = new Date();

  // Phase 1: Clean up expired suppression rows
  await pool.query(
    `DELETE FROM digest_suppression WHERE expires_at < NOW()`,
  );

  if (items.length === 0) {
    return { passed: [], totalItems: 0, suppressedCount: 0, passedCount: 0 };
  }

  // Phase 2: Look up existing suppression rows for these hashes
  const hashes = items.map(computeItemHash);

  const suppressionResult = await pool.query<SuppressionRow>(
    `SELECT hash, source, priority, last_sent_at, expires_at
     FROM digest_suppression
     WHERE hash = ANY($1)`,
    [hashes],
  );

  const suppressionMap = new Map<string, SuppressionRow>();
  for (const row of suppressionResult.rows) {
    suppressionMap.set(row.hash, row);
  }

  // Phase 3: Filter items
  const passed: DigestItem[] = [];
  let suppressedCount = 0;

  for (const item of items) {
    const hash = computeItemHash(item);
    const existing = suppressionMap.get(hash);

    if (existing) {
      const cooldownMs = getCooldownMs(item.priority, config);
      const lastSentMs = existing.last_sent_at.getTime();
      const elapsedMs = now.getTime() - lastSentMs;

      if (elapsedMs < cooldownMs) {
        // Within cooldown — suppress
        suppressedCount++;
        log.debug(
          {
            item_hash: hash,
            source: item.source,
            priority: item.priority,
            reason: "cooldown",
            last_sent_at: existing.last_sent_at.toISOString(),
            cooldown_remaining_ms: cooldownMs - elapsedMs,
          },
          "Item suppressed",
        );
        continue;
      }

      // Cooldown expired — pass through
      log.debug(
        {
          item_hash: hash,
          source: item.source,
          priority: item.priority,
          reason: "cooldown_expired",
        },
        "Item passed (cooldown expired)",
      );
    } else {
      // No suppression row — new item
      log.debug(
        {
          item_hash: hash,
          source: item.source,
          priority: item.priority,
          reason: "new",
        },
        "Item passed (new)",
      );
    }

    passed.push(item);
  }

  const suppressionRatePct =
    items.length > 0
      ? Math.round((suppressedCount / items.length) * 100)
      : 0;

  log.debug(
    {
      total_items: items.length,
      passed: passed.length,
      suppressed: suppressedCount,
      suppression_rate_pct: suppressionRatePct,
    },
    "Suppression run complete",
  );

  return {
    passed,
    totalItems: items.length,
    suppressedCount,
    passedCount: passed.length,
  };
}

/**
 * Record that items have been sent — upsert into digest_suppression
 * with last_sent_at = now and expires_at = now + cooldown.
 * Also accumulate weekly stats for Tier 2 synthesis.
 */
export async function markItemsSent(
  items: DigestItem[],
  pool: Pool,
  config?: DigestConfig,
): Promise<void> {
  const now = new Date();

  if (items.length > 0 && config) {
    // Upsert suppression rows
    for (const item of items) {
      const hash = computeItemHash(item);
      const cooldownMs = getCooldownMs(item.priority, config);
      const expiresAt = new Date(now.getTime() + cooldownMs);

      await pool.query(
        `INSERT INTO digest_suppression (hash, source, priority, last_sent_at, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, NOW())
         ON CONFLICT (hash) DO UPDATE
           SET last_sent_at = EXCLUDED.last_sent_at,
               expires_at = EXCLUDED.expires_at`,
        [hash, item.source, priorityToInt(item.priority), now, expiresAt],
      );
    }
  } else if (items.length > 0) {
    // No config provided — upsert with a 24h default expiry
    const defaultExpiry = new Date(now.getTime() + 86_400_000);
    for (const item of items) {
      const hash = computeItemHash(item);
      await pool.query(
        `INSERT INTO digest_suppression (hash, source, priority, last_sent_at, expires_at, created_at)
         VALUES ($1, $2, $3, $4, $5, NOW())
         ON CONFLICT (hash) DO UPDATE
           SET last_sent_at = EXCLUDED.last_sent_at,
               expires_at = EXCLUDED.expires_at`,
        [hash, item.source, priorityToInt(item.priority), now, defaultExpiry],
      );
    }
  }

  // Update meta (lastDigestAt + weeklyStats)
  const meta = await readDigestMeta(pool);
  meta.lastDigestAt = now.getTime();

  if (!meta.weeklyStats) {
    meta.weeklyStats = {
      itemsBySource: {},
      itemsByPriority: {},
      totalItems: 0,
      digestRuns: 0,
    };
  }

  meta.weeklyStats.digestRuns += 1;
  meta.weeklyStats.totalItems += items.length;

  for (const item of items) {
    meta.weeklyStats.itemsBySource[item.source] =
      (meta.weeklyStats.itemsBySource[item.source] ?? 0) + 1;
    meta.weeklyStats.itemsByPriority[item.priority] =
      (meta.weeklyStats.itemsByPriority[item.priority] ?? 0) + 1;
  }

  await writeDigestMeta(pool, meta);
}

/**
 * Read and reset weekly stats for Tier 2 synthesis.
 */
export async function consumeWeeklyStats(pool: Pool): Promise<WeeklyStats | null> {
  const meta = await readDigestMeta(pool);
  const stats = meta.weeklyStats ?? null;

  if (stats) {
    meta.weeklyStats = {
      itemsBySource: {},
      itemsByPriority: {},
      totalItems: 0,
      digestRuns: 0,
    };
    await writeDigestMeta(pool, meta);
  }

  return stats;
}
