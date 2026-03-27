/** nv dream — memory consolidation commands. */

import {
  heading,
  subheading,
  green,
  yellow,
  gray,
  padRight,
} from "../lib/format.js";

const DAEMON_PORT = 8400;

interface DreamResult {
  topicsProcessed: number;
  bytesBefore: number;
  bytesAfter: number;
  llmTopics: string[];
  durationMs: number;
}

interface DreamStatusResponse {
  lastDreamAt: string | null;
  stats: Record<string, unknown> | null;
  topics: Array<{ topic: string; sizeBytes: number }>;
  totalSizeBytes: number;
}

async function runDream(dryRun: boolean): Promise<void> {
  const queryParam = dryRun ? "?dry_run=true" : "";
  const label = dryRun ? "Dream Dry Run" : "Dream Consolidation";

  heading(label);
  console.log("");
  console.log(gray("Running memory consolidation..."));

  const res = await fetch(`http://localhost:${DAEMON_PORT}/dream${queryParam}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  });

  if (!res.ok) {
    const body = await res.text();
    console.error(`Dream request failed: ${res.status} ${body}`);
    process.exit(1);
  }

  const result = (await res.json()) as DreamResult;

  const beforeKb = Math.round(result.bytesBefore / 1024);
  const afterKb = Math.round(result.bytesAfter / 1024);
  const savedKb = beforeKb - afterKb;
  const pct = beforeKb > 0 ? Math.round((savedKb / beforeKb) * 100) : 0;
  const durationSec = (result.durationMs / 1000).toFixed(1);

  console.log("");
  console.log(`Topics processed:  ${result.topicsProcessed}`);
  console.log(`Before:            ${beforeKb}KB`);
  console.log(`After:             ${afterKb}KB`);
  console.log(`Saved:             ${green(`${savedKb}KB`)} (${pct}%)`);
  console.log(`Duration:          ${durationSec}s`);

  if (result.llmTopics.length > 0) {
    console.log(`LLM compressed:    ${yellow(result.llmTopics.join(", "))}`);
  }

  if (dryRun) {
    console.log("");
    console.log(gray("(dry run — no changes written)"));
  }

  console.log("");
}

async function showStatus(): Promise<void> {
  heading("Memory Dream Status");

  const res = await fetch(`http://localhost:${DAEMON_PORT}/dream/status`);

  if (!res.ok) {
    const body = await res.text();
    console.error(`Dream status request failed: ${res.status} ${body}`);
    process.exit(1);
  }

  const status = (await res.json()) as DreamStatusResponse;

  const totalKb = Math.round(status.totalSizeBytes / 1024);
  const lastDream = status.lastDreamAt
    ? new Date(status.lastDreamAt).toLocaleString()
    : "never";

  console.log("");
  console.log(`Total memory:  ${totalKb}KB`);
  console.log(`Last dream:    ${lastDream}`);

  subheading("\nPer-topic sizes:");
  for (const t of status.topics) {
    const kb = (t.sizeBytes / 1024).toFixed(1);
    const name = padRight(t.topic, 24);
    console.log(`  ${name} ${gray(`${kb}KB`)}`);
  }

  console.log("");
}

export async function dreamCmd(
  subcommand?: string,
  dryRun = false,
): Promise<void> {
  try {
    if (subcommand === "status") {
      await showStatus();
    } else {
      await runDream(dryRun);
    }
  } catch (err) {
    if (
      err instanceof TypeError &&
      (err.message.includes("fetch failed") || err.message.includes("ECONNREFUSED"))
    ) {
      console.error("Cannot connect to daemon on port 8400. Is nova-ts running?");
      process.exit(1);
    }
    throw err;
  }
}
