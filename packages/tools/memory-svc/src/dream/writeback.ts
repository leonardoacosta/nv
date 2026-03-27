import type { DreamResult, RuleStats } from "./types.js";

/** A single changed topic to write back. */
export interface TopicChange {
  topic: string;
  content: string;
}

/** Stats summary stored in the _dream_meta topic. */
export interface DreamMetaPayload {
  lastDreamAt: string;
  stats: {
    topicsProcessed: number;
    bytesBeforeSummed: number;
    bytesAfterSummed: number;
    topicsCompressedByLlm: number;
    rulesApplied: {
      deduped: number;
      datesNormalized: number;
      staleRemoved: number;
      whitespaceFixed: number;
    };
  };
}

/** Diary entry write callback -- injected by the daemon orchestrator. */
export type DiaryWriter = (entry: {
  triggerType: string;
  triggerSource: string;
  channel: string;
  slug: string;
  content: string;
  toolsUsed: string[];
}) => Promise<void>;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async function postWrite(
  memorySvcUrl: string,
  topic: string,
  content: string,
): Promise<void> {
  const resp = await fetch(`${memorySvcUrl}/write`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ topic, content }),
  });
  if (!resp.ok) {
    const body = await resp.text().catch(() => "");
    throw new Error(
      `memory-svc /write failed for topic "${topic}": ${resp.status} ${body}`,
    );
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Write all changed topics back through memory-svc HTTP, update _dream_meta,
 * and write a diary entry summarising the dream run.
 *
 * @param changes       - Topics whose content was modified by rules or LLM.
 * @param memorySvcUrl  - Base URL of memory-svc (e.g. "http://localhost:4101").
 * @param result        - The overall DreamResult (for stats).
 * @param aggregateStats - Summed RuleStats across all topics.
 * @param llmTopics     - Names of topics that went through LLM compression.
 * @param diaryWriter   - Optional callback to write a diary entry. If not
 *                         provided the diary step is skipped.
 */
export async function writeBackTopics(
  changes: TopicChange[],
  memorySvcUrl: string,
  result: DreamResult,
  aggregateStats: RuleStats,
  llmTopics: string[],
  diaryWriter?: DiaryWriter,
): Promise<void> {
  // 1. Write each changed topic
  for (const { topic, content } of changes) {
    await postWrite(memorySvcUrl, topic, content);
  }

  // 2. Write _dream_meta topic
  const metaPayload: DreamMetaPayload = {
    lastDreamAt: new Date().toISOString(),
    stats: {
      topicsProcessed: result.topicsProcessed,
      bytesBeforeSummed: result.bytesBefore,
      bytesAfterSummed: result.bytesAfter,
      topicsCompressedByLlm: llmTopics.length,
      rulesApplied: {
        deduped: aggregateStats.dedupedLines,
        datesNormalized: aggregateStats.datesNormalized,
        staleRemoved: aggregateStats.stalePathsRemoved,
        whitespaceFixed: aggregateStats.whitespaceFixed,
      },
    },
  };

  await postWrite(memorySvcUrl, "_dream_meta", JSON.stringify(metaPayload, null, 2));

  // 3. Write diary entry
  if (diaryWriter) {
    const kbBefore = (result.bytesBefore / 1024).toFixed(0);
    const kbAfter = (result.bytesAfter / 1024).toFixed(0);
    const slug = `Dream: ${result.topicsProcessed} topics, ${kbBefore}KB -> ${kbAfter}KB`;

    await diaryWriter({
      triggerType: "dream",
      triggerSource: "dream-consolidation",
      channel: "system",
      slug,
      content: [
        `Dream consolidation completed in ${result.durationMs}ms.`,
        `Topics processed: ${result.topicsProcessed}`,
        `Size: ${kbBefore}KB -> ${kbAfter}KB`,
        llmTopics.length > 0
          ? `LLM-compressed topics: ${llmTopics.join(", ")}`
          : "No topics required LLM compression.",
        `Rules applied: ${aggregateStats.dedupedLines} deduped, ${aggregateStats.datesNormalized} dates normalized, ${aggregateStats.stalePathsRemoved} stale paths removed, ${aggregateStats.whitespaceFixed} whitespace fixes`,
      ].join("\n"),
      toolsUsed: [],
    });
  }
}
