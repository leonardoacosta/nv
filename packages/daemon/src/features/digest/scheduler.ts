import type { Pool } from "pg";
import type { Logger } from "pino";
import type { Config } from "../../config.js";
import { createAgentQuery } from "../../brain/query-factory.js";
import type { TelegramAdapter } from "../../channels/telegram.js";
import { gatherDigest } from "./gather.js";
import { classifyItems } from "./classify.js";
import { suppressItems, markItemsSent, consumeWeeklyStats } from "./suppress.js";
import { formatDigest, formatWeeklySynthesis } from "./format.js";
import { checkP0 } from "./realtime.js";
import { isQuietHours } from "../../lib/quiet-hours.js";
import { writeEntry } from "../diary/writer.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface DigestSchedulerDeps {
  pool: Pool;
  logger: Logger;
  telegram: TelegramAdapter | null;
  telegramChatId: string | null;
  config: Config;
}

// ─── Tier 1 Runner ────────────────────────────────────────────────────────────

async function runTier1Digest(deps: DigestSchedulerDeps): Promise<void> {
  const { pool, logger, telegram, telegramChatId, config } = deps;

  if (!telegram || !telegramChatId) return;

  logger.info("Digest Tier 1: gathering sources");
  const gatherResult = await gatherDigest({ pool, logger });

  logger.info(
    { sourcesStatus: gatherResult.sourcesStatus },
    "Digest Tier 1: classifying items",
  );
  const classified = classifyItems(gatherResult);

  if (classified.length === 0) {
    logger.debug("Digest Tier 1: nothing to classify");
    return;
  }

  const suppressResult = await suppressItems(classified, pool, config.digest);
  const { passed: unsuppressed } = suppressResult;

  if (unsuppressed.length === 0) {
    logger.debug("Digest Tier 1: nothing new to report");
    return;
  }

  const { text, keyboard } = formatDigest(unsuppressed, "thin");

  if (!text) {
    logger.debug("Digest Tier 1: formatted output empty");
    return;
  }

  try {
    await telegram.sendMessage(telegramChatId, text, {
      parseMode: "Markdown",
      disablePreview: true,
      ...(keyboard ? { keyboard } : {}),
    });

    await markItemsSent(unsuppressed, pool, config.digest);
    logger.info({ itemCount: unsuppressed.length }, "Digest Tier 1: sent");

    // Write diary entry for observability
    void writeEntry({
      triggerType: "digest_run",
      triggerSource: "scheduler",
      channel: "telegram",
      slug: `digest:tier1:${new Date().toISOString().slice(0, 10)}`,
      content: `Tier 1 digest: ${unsuppressed.length} items sent (${suppressResult.suppressedCount} suppressed of ${suppressResult.totalItems} total, ${Math.round((suppressResult.suppressedCount / Math.max(suppressResult.totalItems, 1)) * 100)}% suppression rate)`,
      toolsUsed: [],
    });
  } catch (err) {
    logger.warn({ err }, "Digest Tier 1: failed to send");
  }
}

// ─── Tier 2 Runner ────────────────────────────────────────────────────────────

const TIER2_SYSTEM_PROMPT = `You are Nova's weekly digest synthesizer. Analyze the weekly digest statistics and produce a concise 200-word weekly trend summary.

Structure:
1. Key patterns observed (what sources generated the most items, what priorities dominated)
2. Notable trends (increasing/decreasing activity, new patterns)
3. Three suggested focus areas for the coming week

Be direct, actionable, and concise. No fluff.`;

const TIER2_TIMEOUT_MS = 30_000;

async function runTier2Digest(deps: DigestSchedulerDeps): Promise<void> {
  const { pool, logger, telegram, telegramChatId, config } = deps;

  if (!telegram || !telegramChatId) return;

  logger.info("Digest Tier 2: reading weekly stats");
  const stats = await consumeWeeklyStats(pool);

  if (!stats || stats.totalItems === 0) {
    logger.info("Digest Tier 2: no weekly stats — skipping");
    return;
  }

  const prompt = [
    "Weekly digest statistics:",
    `- Total items surfaced: ${stats.totalItems}`,
    `- Digest runs: ${stats.digestRuns}`,
    `- Items by source: ${JSON.stringify(stats.itemsBySource)}`,
    `- Items by priority: ${JSON.stringify(stats.itemsByPriority)}`,
    "",
    "Produce a 200-word weekly trend summary with 3 focus areas.",
  ].join("\n");

  try {
    let synthesisText = "";

    // Check if we have the Vercel gateway key for Agent SDK
    const gatewayKey = config.vercelGatewayKey;

    if (gatewayKey) {
      const queryResult = await createAgentQuery({
        prompt,
        systemPrompt: TIER2_SYSTEM_PROMPT,
        maxTurns: 1,
        timeoutMs: TIER2_TIMEOUT_MS,
        allowedTools: [],
        gatewayKey,
      });
      synthesisText = queryResult.text;
    } else {
      // Fallback: produce a static summary without LLM
      synthesisText = buildStaticWeeklySummary(stats);
    }

    if (!synthesisText) {
      synthesisText = buildStaticWeeklySummary(stats);
    }

    const { text } = formatWeeklySynthesis(synthesisText);

    await telegram.sendMessage(telegramChatId, text, {
      parseMode: "Markdown",
      disablePreview: true,
    });

    logger.info("Digest Tier 2: weekly synthesis sent");
  } catch (err) {
    logger.warn({ err }, "Digest Tier 2: synthesis failed — sending static summary");

    // Fallback to static summary
    const fallback = buildStaticWeeklySummary(stats);
    const { text } = formatWeeklySynthesis(fallback);

    try {
      await telegram.sendMessage(telegramChatId, text, {
        parseMode: "Markdown",
        disablePreview: true,
      });
    } catch (sendErr) {
      logger.warn({ err: sendErr }, "Digest Tier 2: fallback send also failed");
    }
  }
}

function buildStaticWeeklySummary(stats: { itemsBySource: Record<string, number>; itemsByPriority: Record<string, number>; totalItems: number; digestRuns: number }): string {
  const sourceLines = Object.entries(stats.itemsBySource)
    .sort(([, a], [, b]) => b - a)
    .map(([source, count]) => `  - ${source}: ${count}`)
    .join("\n");

  const priorityLines = Object.entries(stats.itemsByPriority)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([priority, count]) => `  - ${priority}: ${count}`)
    .join("\n");

  return [
    `Weekly Summary (${stats.digestRuns} digest runs)`,
    "",
    `Total items: ${stats.totalItems}`,
    "",
    "By source:",
    sourceLines || "  (none)",
    "",
    "By priority:",
    priorityLines || "  (none)",
    "",
    "_AI synthesis unavailable -- showing raw counts._",
  ].join("\n");
}

// ─── Scheduler ────────────────────────────────────────────────────────────────

const POLL_INTERVAL_MS = 60_000; // 60s poll for Tier 1 / Tier 2

/**
 * Start the digest scheduler with three independent loops:
 * 1. Tier 1 thin digest: 7am, 12pm, 5pm weekdays
 * 2. Tier 2 weekly LLM: Monday 9am
 * 3. P0 real-time: every 5 minutes (configurable)
 *
 * Returns a cleanup function that clears all intervals.
 */
export function startDigestScheduler(deps: DigestSchedulerDeps): () => void {
  const { logger, config } = deps;
  const digestConfig = config.digest;

  // ── Tier 1 state ──────────────────────────────────────────────────────────

  const lastTier1Dates = new Map<number, string>();

  const tier1Interval = setInterval(() => {
    const now = new Date();
    const hour = now.getHours();
    const day = now.getDay(); // 0=Sun, 6=Sat
    const todayStr = now.toISOString().slice(0, 10);

    // Skip weekends
    if (day === 0 || day === 6) return;

    // Skip quiet hours
    if (isQuietHours(new Date(), digestConfig.quietStart, digestConfig.quietEnd)) return;

    // Check if current hour is a Tier 1 hour
    if (!digestConfig.tier1Hours.includes(hour)) return;

    // Dedup: already fired this hour today?
    const key = hour;
    if (lastTier1Dates.get(key) === todayStr) return;

    lastTier1Dates.set(key, todayStr);

    logger.info({ hour, date: todayStr }, "Digest Tier 1 scheduler firing");

    void runTier1Digest(deps).catch((err: unknown) => {
      logger.error({ err }, "Digest Tier 1 run failed");
    });
  }, POLL_INTERVAL_MS);

  // ── Tier 2 state ──────────────────────────────────────────────────────────

  let lastTier2Date: string | null = null;

  const tier2Interval = setInterval(() => {
    const now = new Date();
    const day = now.getDay();
    const hour = now.getHours();
    const todayStr = now.toISOString().slice(0, 10);

    // Only fire on the configured day (default: Monday = 1) at the configured hour
    if (day !== digestConfig.tier2Day) return;
    if (hour !== digestConfig.tier2Hour) return;

    // Skip quiet hours for Tier 2
    if (isQuietHours(new Date(), digestConfig.quietStart, digestConfig.quietEnd)) return;

    // Dedup
    if (lastTier2Date === todayStr) return;
    lastTier2Date = todayStr;

    logger.info({ day, hour, date: todayStr }, "Digest Tier 2 scheduler firing");

    void runTier2Digest(deps).catch((err: unknown) => {
      logger.error({ err }, "Digest Tier 2 run failed");
    });
  }, POLL_INTERVAL_MS);

  // ── P0 real-time ──────────────────────────────────────────────────────────

  const p0Interval = setInterval(() => {
    // P0 bypasses quiet hours — always runs
    void checkP0(deps).catch((err: unknown) => {
      logger.error({ err }, "Digest P0 real-time check failed");
    });
  }, digestConfig.realtimeIntervalMs);

  // ── Cleanup ───────────────────────────────────────────────────────────────

  return () => {
    clearInterval(tier1Interval);
    clearInterval(tier2Interval);
    clearInterval(p0Interval);
  };
}

// Re-export runners for on-demand /digest command
export { runTier1Digest, runTier2Digest };
