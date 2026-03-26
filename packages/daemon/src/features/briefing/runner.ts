import { gatherContext, synthesizeBriefing } from "./synthesizer.js";
import type { BriefingDeps } from "./synthesizer.js";

// ─── BriefingRow ──────────────────────────────────────────────────────────────

interface BriefingRow {
  id: string;
  generated_at: Date;
}

// ─── runMorningBriefing ───────────────────────────────────────────────────────

/**
 * Orchestrates the morning briefing pipeline:
 * 1. Gather context (obligations, memory, messages)
 * 2. Synthesize with Claude
 * 3. Persist to the `briefings` table
 *
 * On error, logs and re-throws so the caller can decide how to handle it.
 */
export async function runMorningBriefing(deps: BriefingDeps): Promise<void> {
  const { pool, logger } = deps;

  logger.info("Morning briefing: gathering context");
  const context = await gatherContext(deps);

  logger.info(
    { sourcesStatus: context.sourcesStatus },
    "Morning briefing: synthesizing",
  );
  const synthesis = await synthesizeBriefing(context, deps);

  const result = await pool.query<BriefingRow>(
    `INSERT INTO briefings (content, sources_status, suggested_actions)
     VALUES ($1, $2, $3)
     RETURNING id, generated_at`,
    [
      synthesis.content,
      JSON.stringify(context.sourcesStatus),
      JSON.stringify(synthesis.suggestedActions),
    ],
  );

  const row = result.rows[0];
  if (!row) {
    throw new Error("Briefing INSERT did not return a row");
  }

  logger.info(
    { briefingId: row.id, generatedAt: row.generated_at.toISOString() },
    "Morning briefing generated and persisted",
  );
}
