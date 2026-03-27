import { createHash } from "node:crypto";
import type { Pool } from "pg";
import type { DigestConfig } from "../../config.js";
import type { DigestItem, Priority } from "./classify.js";

// ─── Types ────────────────────────────────────────────────────────────────────

interface DigestMeta {
  sentHashes: Record<string, number>; // hash -> unix timestamp (ms) of when it was sent
  lastDigestAt: number | null;
  weeklyStats?: WeeklyStats;
}

export interface WeeklyStats {
  itemsBySource: Record<string, number>;
  itemsByPriority: Record<string, number>;
  totalItems: number;
  digestRuns: number;
}

const DIGEST_META_TOPIC = "_digest_meta";

// ─── State Persistence ────────────────────────────────────────────────────────

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
      const parsed = JSON.parse(result.rows[0].content) as DigestMeta;
      return {
        sentHashes: parsed.sentHashes ?? {},
        lastDigestAt: parsed.lastDigestAt ?? null,
        weeklyStats: parsed.weeklyStats,
      };
    }
  } catch {
    // Malformed JSON or missing row — start fresh
  }

  return { sentHashes: {}, lastDigestAt: null };
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

export async function suppressItems(
  items: DigestItem[],
  pool: Pool,
  config: DigestConfig,
): Promise<DigestItem[]> {
  const meta = await readDigestMeta(pool);
  const now = Date.now();

  // Phase 1: Prune hashes older than hashTtlMs (48h default)
  const prunedHashes: Record<string, number> = {};
  for (const [hash, timestamp] of Object.entries(meta.sentHashes)) {
    if (now - timestamp < config.hashTtlMs) {
      prunedHashes[hash] = timestamp;
    }
  }
  meta.sentHashes = prunedHashes;

  // Phase 2: Filter items based on cooldown
  const passed: DigestItem[] = [];

  for (const item of items) {
    const hash = computeItemHash(item);
    const lastSent = meta.sentHashes[hash];

    if (lastSent !== undefined) {
      const cooldown = getCooldownMs(item.priority, config);
      if (now - lastSent < cooldown) {
        continue; // Still within cooldown — suppress
      }
    }

    passed.push(item);
  }

  return passed;
}

/**
 * Record that items have been sent — update the sentHashes timestamps.
 * Also accumulate weekly stats for Tier 2 synthesis.
 */
export async function markItemsSent(
  items: DigestItem[],
  pool: Pool,
): Promise<void> {
  const meta = await readDigestMeta(pool);
  const now = Date.now();

  for (const item of items) {
    const hash = computeItemHash(item);
    meta.sentHashes[hash] = now;
  }

  meta.lastDigestAt = now;

  // Accumulate weekly stats
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
