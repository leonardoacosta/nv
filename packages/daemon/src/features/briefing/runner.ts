import { gatherContext, synthesizeBriefing } from "./synthesizer.js";
import type { BriefingDeps } from "./synthesizer.js";

// ─── BriefingRow ──────────────────────────────────────────────────────────────

interface BriefingRow {
  id: string;
  generated_at: Date;
}

// ─── Telegram helpers ─────────────────────────────────────────────────────────

const TELEGRAM_MAX_LEN = 4096;
const DASHBOARD_SUFFIX = "\n\n... [view full briefing on dashboard]";

function truncateForTelegram(content: string): string {
  if (content.length <= TELEGRAM_MAX_LEN) return content;
  return content.slice(0, TELEGRAM_MAX_LEN - DASHBOARD_SUFFIX.length) + DASHBOARD_SUFFIX;
}

// ─── runMorningBriefing ───────────────────────────────────────────────────────

/**
 * Orchestrates the morning briefing pipeline:
 * 1. Gather context (obligations, memory, messages, calendar, diary)
 * 2. Synthesize with Claude
 * 3. Persist to the `briefings` table
 * 4. Send to Telegram (if configured)
 *
 * Returns the briefing row id and generated_at for use by the HTTP endpoint.
 * On error, logs and re-throws so the caller can decide how to handle it.
 */
export async function runMorningBriefing(deps: BriefingDeps): Promise<BriefingRow> {
  const { pool, logger } = deps;

  logger.info("Morning briefing: gathering context");
  const context = await gatherContext(deps);

  logger.info(
    { sourcesStatus: context.sourcesStatus },
    "Morning briefing: synthesizing",
  );
  const synthesis = await synthesizeBriefing(context, deps);

  const result = await pool.query<BriefingRow>(
    `INSERT INTO briefings (content, sources_status, suggested_actions, blocks)
     VALUES ($1, $2, $3, $4)
     RETURNING id, generated_at`,
    [
      synthesis.content,
      JSON.stringify(context.sourcesStatus),
      JSON.stringify(synthesis.suggestedActions),
      synthesis.blocks !== null ? JSON.stringify(synthesis.blocks) : null,
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

  // Send to Telegram (non-blocking — failures are logged but never fail the run)
  if (deps.telegram && deps.telegramChatId) {
    try {
      const telegramContent = truncateForTelegram(synthesis.content);
      await deps.telegram.sendMessage(deps.telegramChatId, telegramContent, {
        parseMode: "Markdown",
        disablePreview: true,
      });
      logger.info({ briefingId: row.id }, "Morning briefing sent to Telegram");
    } catch (err) {
      logger.warn({ err, briefingId: row.id }, "Failed to send briefing to Telegram — skipping");
    }
  }

  return row;
}
