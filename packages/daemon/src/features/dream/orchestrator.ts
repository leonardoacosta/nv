import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { db, memory } from "@nova/db";
import { eq } from "drizzle-orm";
import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";

import { logger } from "../../logger.js";
import { writeEntry } from "../diary/index.js";
import type { DreamResult, TopicStats, RuleResult } from "./types.js";

// ── Orient Phase ──────────────────────────────────────────────────────────────

interface DreamOrientation {
  topics: TopicStats[];
  totalSizeBytes: number;
  timestamp: Date;
}

async function orient(): Promise<DreamOrientation> {
  const rows = await db.select().from(memory);

  const topics: TopicStats[] = rows.map((row) => ({
    topic: row.topic,
    sizeBytes: Buffer.byteLength(row.content, "utf-8"),
    lineCount: row.content.split("\n").length,
    updatedAt: row.updatedAt,
  }));

  const totalSizeBytes = topics.reduce((sum, t) => sum + t.sizeBytes, 0);

  return { topics, totalSizeBytes, timestamp: new Date() };
}

// ── Deterministic Rules Phase ─────────────────────────────────────────────────

function applyRules(
  content: string,
  updatedAt: Date,
  topicMaxKb: number,
): RuleResult {
  let result = content;
  let dedupedLines = 0;
  let datesNormalized = 0;
  let stalePathsRemoved = 0;
  let whitespaceFixed = 0;

  // Rule 1: Deduplicate exact duplicate lines
  {
    const lines = result.split("\n");
    const seen = new Set<string>();
    const deduped: string[] = [];

    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed === "") {
        deduped.push(line);
        continue;
      }
      if (seen.has(trimmed)) {
        dedupedLines++;
        continue;
      }
      seen.add(trimmed);
      deduped.push(line);
    }

    // Near-duplicate detection (Levenshtein < 10% of line length, min 20 chars)
    const filtered: string[] = [];
    for (const line of deduped) {
      const trimmed = line.trim();
      if (trimmed.length < 20) {
        filtered.push(line);
        continue;
      }

      let isDuplicate = false;
      for (let i = filtered.length - 1; i >= Math.max(0, filtered.length - 20); i--) {
        const existing = filtered[i]!.trim();
        if (existing.length < 20) continue;

        const threshold = Math.floor(Math.max(trimmed.length, existing.length) * 0.1);
        if (Math.abs(trimmed.length - existing.length) > threshold) continue;

        const dist = levenshtein(trimmed, existing);
        if (dist < threshold) {
          // Keep the longer variant
          if (trimmed.length > existing.length) {
            filtered[i] = line;
          }
          isDuplicate = true;
          dedupedLines++;
          break;
        }
      }

      if (!isDuplicate) {
        filtered.push(line);
      }
    }

    result = filtered.join("\n");
  }

  // Rule 2: Date normalization — convert relative dates to absolute
  {
    const ref = updatedAt;
    const replacements: Array<[RegExp, () => string]> = [
      [/\byesterday\b/gi, () => {
        const d = new Date(ref);
        d.setDate(d.getDate() - 1);
        return d.toISOString().slice(0, 10);
      }],
      [/\btoday\b/gi, () => ref.toISOString().slice(0, 10)],
      [/\bthis morning\b/gi, () => ref.toISOString().slice(0, 10)],
      [/\blast week\b/gi, () => {
        const d = new Date(ref);
        d.setDate(d.getDate() - 7);
        return `week of ${d.toISOString().slice(0, 10)}`;
      }],
      [/\blast month\b/gi, () => {
        const d = new Date(ref);
        d.setMonth(d.getMonth() - 1);
        return d.toISOString().slice(0, 7);
      }],
      [/\brecently\b/gi, () => `around ${ref.toISOString().slice(0, 10)}`],
      [/\ba few days ago\b/gi, () => {
        const d = new Date(ref);
        d.setDate(d.getDate() - 3);
        return `around ${d.toISOString().slice(0, 10)}`;
      }],
    ];

    for (const [pattern, replacer] of replacements) {
      const before = result;
      result = result.replace(pattern, replacer);
      if (result !== before) {
        const matches = before.match(pattern);
        datesNormalized += matches?.length ?? 0;
      }
    }
  }

  // Rule 3: Whitespace cleanup
  {
    const before = result;

    // Trim trailing whitespace per line
    result = result
      .split("\n")
      .map((line) => line.trimEnd())
      .join("\n");

    // Collapse 3+ consecutive blank lines to 2
    result = result.replace(/\n{4,}/g, "\n\n\n");

    // Remove leading/trailing blank lines
    result = result.replace(/^\n+/, "").replace(/\n+$/, "");

    if (result !== before) {
      whitespaceFixed++;
    }
  }

  // Rule 4: Stale path removal
  {
    const home = homedir();
    const lines = result.split("\n");
    const filtered: string[] = [];

    for (const line of lines) {
      const trimmed = line.trim();
      // Match lines where a path is the primary content
      const pathMatch = trimmed.match(
        /^(?:-\s+)?(~\/[^\s]+|\/home\/[^\s]+|packages\/[^\s]+|apps\/[^\s]+)$/,
      );

      if (pathMatch?.[1]) {
        const rawPath = pathMatch[1];
        const expandedPath = rawPath.startsWith("~/")
          ? rawPath.replace("~", home)
          : rawPath;

        if (!existsSync(expandedPath)) {
          stalePathsRemoved++;
          continue;
        }
      }

      filtered.push(line);
    }

    result = filtered.join("\n");
  }

  // Rule 5: Budget check
  const needsLlm = Buffer.byteLength(result, "utf-8") > topicMaxKb * 1024;

  return {
    content: result,
    needsLlm,
    stats: { dedupedLines, datesNormalized, stalePathsRemoved, whitespaceFixed },
  };
}

// ── LLM Compression Phase ─────────────────────────────────────────────────────

async function compressTopic(
  topic: string,
  content: string,
  targetKb: number,
): Promise<string | null> {
  const gatewayKey = process.env["VERCEL_GATEWAY_KEY"];
  if (!gatewayKey) {
    logger.warn("VERCEL_GATEWAY_KEY not set — skipping LLM compression");
    return null;
  }

  const systemPrompt =
    `You are a memory compressor. Compress the following memory topic to under ${targetKb}KB. ` +
    "Preserve: recent decisions, active projects, key relationships, dates, names, technical details. " +
    "Remove: stale context, resolved issues, outdated patterns, redundant information. " +
    "Output only the compressed content -- no preamble, no explanation.";

  try {
    let resultText = "";

    const queryStream = query({
      prompt: content,
      options: {
        systemPrompt,
        allowedTools: [],
        permissionMode: "bypassPermissions",
        allowDangerouslySkipPermissions: true,
        maxTurns: 1,
        env: {
          ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
          ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
        },
      },
    });

    const timeoutPromise = new Promise<null>((resolve) => {
      setTimeout(() => resolve(null), 60_000);
    });

    const streamPromise = (async () => {
      for await (const sdkMsg of queryStream as AsyncIterable<SDKMessage>) {
        if (sdkMsg.type === "result" && sdkMsg.subtype === "success") {
          resultText = sdkMsg.result;
        }
      }
      return resultText;
    })();

    const result = await Promise.race([streamPromise, timeoutPromise]);

    if (!result) {
      logger.warn({ topic }, "LLM compression timed out or returned empty");
      return null;
    }

    return result;
  } catch (err) {
    logger.warn({ err, topic }, "LLM compression failed — keeping rules-phase result");
    return null;
  }
}

// ── Writeback Phase ───────────────────────────────────────────────────────────

const MEMORY_SVC_PORT = 4101;

async function writeBackTopic(topic: string, content: string): Promise<void> {
  const url = `http://localhost:${MEMORY_SVC_PORT}/write`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ topic, content }),
  });

  if (!res.ok) {
    throw new Error(`Writeback failed for topic "${topic}": ${res.status} ${res.statusText}`);
  }
}

async function writeDreamMeta(stats: {
  topicsProcessed: number;
  bytesBefore: number;
  bytesAfter: number;
  llmTopics: string[];
  rulesApplied: {
    deduped: number;
    datesNormalized: number;
    staleRemoved: number;
    whitespaceFixed: number;
  };
}): Promise<void> {
  const meta = JSON.stringify({
    lastDreamAt: new Date().toISOString(),
    stats: {
      topicsProcessed: stats.topicsProcessed,
      bytesBeforeSummed: stats.bytesBefore,
      bytesAfterSummed: stats.bytesAfter,
      topicsCompressedByLlm: stats.llmTopics.length,
      rulesApplied: stats.rulesApplied,
    },
  });

  await writeBackTopic("_dream_meta", meta);
}

// ── Main Orchestrator ─────────────────────────────────────────────────────────

export interface DreamRunConfig {
  topicMaxKb: number;
  dryRun?: boolean;
}

export async function runDream(config: DreamRunConfig): Promise<DreamResult> {
  const startMs = Date.now();

  // Phase 1: Orient
  const orientation = await orient();

  logger.info(
    {
      topicCount: orientation.topics.length,
      totalSizeKb: Math.round(orientation.totalSizeBytes / 1024),
    },
    "Dream orient phase complete",
  );

  // Phase 2: Rules + Phase 3: LLM Compress
  const changes: Array<{ topic: string; content: string }> = [];
  const llmTopics: string[] = [];
  let bytesBefore = 0;
  let bytesAfter = 0;
  let totalDeduped = 0;
  let totalDatesNormalized = 0;
  let totalStaleRemoved = 0;
  let totalWhitespaceFixed = 0;

  // Read full content for each topic
  const allRows = await db.select().from(memory);
  const contentMap = new Map(allRows.map((r) => [r.topic, r]));

  for (const topicStats of orientation.topics) {
    // Skip the _dream_meta topic itself
    if (topicStats.topic === "_dream_meta") continue;

    const row = contentMap.get(topicStats.topic);
    if (!row) continue;

    const originalContent = row.content;
    const originalSize = Buffer.byteLength(originalContent, "utf-8");
    bytesBefore += originalSize;

    // Apply deterministic rules
    const ruleResult = applyRules(originalContent, row.updatedAt, config.topicMaxKb);

    totalDeduped += ruleResult.stats.dedupedLines;
    totalDatesNormalized += ruleResult.stats.datesNormalized;
    totalStaleRemoved += ruleResult.stats.stalePathsRemoved;
    totalWhitespaceFixed += ruleResult.stats.whitespaceFixed;

    let finalContent = ruleResult.content;

    // Phase 3: LLM compression for oversize topics
    if (ruleResult.needsLlm) {
      const compressed = await compressTopic(
        topicStats.topic,
        ruleResult.content,
        config.topicMaxKb,
      );
      if (compressed) {
        finalContent = compressed;
        llmTopics.push(topicStats.topic);
      }
    }

    const finalSize = Buffer.byteLength(finalContent, "utf-8");
    bytesAfter += finalSize;

    // Only write back if content actually changed
    if (finalContent !== originalContent) {
      changes.push({ topic: topicStats.topic, content: finalContent });
    }
  }

  logger.info(
    {
      changedTopics: changes.length,
      llmTopics: llmTopics.length,
      bytesBefore,
      bytesAfter,
    },
    "Dream rules + compress phases complete",
  );

  // Phase 4: Writeback
  if (!config.dryRun) {
    for (const change of changes) {
      try {
        await writeBackTopic(change.topic, change.content);
      } catch (err) {
        logger.error({ err, topic: change.topic }, "Dream writeback failed for topic");
      }
    }

    // Write _dream_meta
    await writeDreamMeta({
      topicsProcessed: orientation.topics.filter((t) => t.topic !== "_dream_meta").length,
      bytesBefore,
      bytesAfter,
      llmTopics,
      rulesApplied: {
        deduped: totalDeduped,
        datesNormalized: totalDatesNormalized,
        staleRemoved: totalStaleRemoved,
        whitespaceFixed: totalWhitespaceFixed,
      },
    });

    // Write diary entry
    const savedKb = Math.round((bytesBefore - bytesAfter) / 1024);
    void writeEntry({
      triggerType: "dream",
      triggerSource: "daemon",
      channel: "system",
      slug: `Dream: ${orientation.topics.length} topics, ${Math.round(bytesBefore / 1024)}KB -> ${Math.round(bytesAfter / 1024)}KB`,
      content: `Consolidated ${changes.length} topics. Saved ${savedKb}KB. LLM compressed: ${llmTopics.join(", ") || "none"}.`,
      toolsUsed: [],
    });
  }

  const durationMs = Date.now() - startMs;

  logger.info(
    { durationMs, dryRun: config.dryRun ?? false },
    "Dream cycle complete",
  );

  return {
    topicsProcessed: orientation.topics.filter((t) => t.topic !== "_dream_meta").length,
    bytesBefore,
    bytesAfter,
    llmTopics,
    durationMs,
  };
}

// ── Status Helper ─────────────────────────────────────────────────────────────

export interface DreamStatus {
  lastDreamAt: string | null;
  stats: Record<string, unknown> | null;
  topics: Array<{ topic: string; sizeBytes: number }>;
  totalSizeBytes: number;
}

export async function getDreamStatus(): Promise<DreamStatus> {
  // Read _dream_meta
  const metaRow = await db
    .select()
    .from(memory)
    .where(eq(memory.topic, "_dream_meta"))
    .limit(1);

  let lastDreamAt: string | null = null;
  let stats: Record<string, unknown> | null = null;

  if (metaRow[0]) {
    try {
      const parsed = JSON.parse(metaRow[0].content) as {
        lastDreamAt?: string;
        stats?: Record<string, unknown>;
      };
      lastDreamAt = parsed.lastDreamAt ?? null;
      stats = parsed.stats ?? null;
    } catch {
      // malformed JSON — ignore
    }
  }

  // Read all topics with sizes
  const allRows = await db.select().from(memory);
  const topics = allRows
    .filter((r) => r.topic !== "_dream_meta")
    .map((r) => ({
      topic: r.topic,
      sizeBytes: Buffer.byteLength(r.content, "utf-8"),
    }))
    .sort((a, b) => b.sizeBytes - a.sizeBytes);

  const totalSizeBytes = topics.reduce((sum, t) => sum + t.sizeBytes, 0);

  return { lastDreamAt, stats, topics, totalSizeBytes };
}

// ── Utility Functions ─────────────────────────────────────────────────────────

function levenshtein(a: string, b: string): number {
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;
  if (a === b) return 0;

  const lenA = a.length;
  const lenB = b.length;

  let prev = new Array<number>(lenB + 1);
  let curr = new Array<number>(lenB + 1);

  for (let j = 0; j <= lenB; j++) prev[j] = j;

  for (let i = 1; i <= lenA; i++) {
    curr[0] = i;
    for (let j = 1; j <= lenB; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      curr[j] = Math.min(
        prev[j]! + 1,
        curr[j - 1]! + 1,
        prev[j - 1]! + cost,
      );
    }
    [prev, curr] = [curr, prev];
  }

  return prev[lenB]!;
}
