import type { DreamResult } from "../../features/dream/types.js";

const DAEMON_PORT = 8400;

interface DreamStatusResponse {
  lastDreamAt: string | null;
  stats: Record<string, unknown> | null;
  topics: Array<{ topic: string; sizeBytes: number }>;
  totalSizeBytes: number;
}

/**
 * /dream — run a dream cycle and format the result for Telegram.
 */
export async function buildDreamReply(): Promise<string> {
  const res = await fetch(`http://localhost:${DAEMON_PORT}/dream`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Dream request failed: ${res.status} ${body}`);
  }

  const result = (await res.json()) as DreamResult;

  const beforeKb = Math.round(result.bytesBefore / 1024);
  const afterKb = Math.round(result.bytesAfter / 1024);
  const savedKb = beforeKb - afterKb;
  const pct = beforeKb > 0 ? Math.round((savedKb / beforeKb) * 100) : 0;
  const durationSec = (result.durationMs / 1000).toFixed(1);

  const lines = [
    "Dream Consolidation Complete",
    "\u2500".repeat(32),
    `  Topics processed: ${result.topicsProcessed}`,
    `  Before: ${beforeKb}KB`,
    `  After: ${afterKb}KB`,
    `  Saved: ${savedKb}KB (${pct}%)`,
    `  Duration: ${durationSec}s`,
  ];

  if (result.llmTopics.length > 0) {
    lines.push(`  LLM compressed: ${result.llmTopics.join(", ")}`);
  }

  return lines.join("\n");
}

/**
 * /dream status — show per-topic sizes and last dream info.
 */
export async function buildDreamStatusReply(): Promise<string> {
  const res = await fetch(`http://localhost:${DAEMON_PORT}/dream/status`);

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Dream status request failed: ${res.status} ${body}`);
  }

  const status = (await res.json()) as DreamStatusResponse;

  const totalKb = Math.round(status.totalSizeBytes / 1024);
  const lastDream = status.lastDreamAt
    ? new Date(status.lastDreamAt).toLocaleString()
    : "never";

  const lines = [
    `Memory Status (${totalKb}KB total)`,
    "\u2500".repeat(32),
    `  Last dream: ${lastDream}`,
    "",
    "Per-topic sizes:",
  ];

  for (const t of status.topics) {
    const kb = (t.sizeBytes / 1024).toFixed(1);
    lines.push(`  ${t.topic}: ${kb}KB`);
  }

  return lines.join("\n");
}
