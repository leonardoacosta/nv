import type { Pool } from "pg";
import type { Logger } from "pino";
import type { TelegramAdapter } from "../../channels/telegram.js";
import type { ProactiveWatcherConfig } from "./types.js";
import { watcherKeyboard } from "./callbacks.js";
import type { ObligationStore } from "../obligations/store.js";
import { ObligationStatus } from "../obligations/types.js";
import { isQuietHours } from "../../lib/quiet-hours.js";

// ─── Row shape returned by pg (snake_case columns) ───────────────────────────

interface ObligationRow {
  id: string;
  detected_action: string;
  owner: string;
  status: string;
  priority: number;
  project_code: string | null;
  source_channel: string;
  source_message: string | null;
  deadline: Date | null;
  last_attempt_at: Date | null;
  created_at: Date;
  updated_at: Date;
}

// ─── Scan type ────────────────────────────────────────────────────────────────

export type ScanType = "overdue" | "stale" | "approaching";

// ─── formatReminderCard ───────────────────────────────────────────────────────

/**
 * Formats an obligation as an HTML Telegram reminder card.
 *
 * Format:
 *   <b>[OVERDUE]</b> Deploy auth service by Friday
 *   Status: in_progress
 *   Overdue by: 2 days
 *   Project: OO         (omitted when project_code is null)
 */
export function formatReminderCard(
  row: ObligationRow,
  scanType: ScanType,
  now: Date = new Date(),
): string {
  const badge = scanType.toUpperCase();
  const lines: string[] = [
    `<b>[${badge}]</b> ${row.detected_action}`,
    `Status: ${row.status}`,
  ];

  switch (scanType) {
    case "overdue": {
      if (row.deadline !== null) {
        const diffMs = now.getTime() - row.deadline.getTime();
        const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
        if (diffHours >= 24) {
          const days = Math.floor(diffHours / 24);
          lines.push(`Overdue by: ${days} day${days !== 1 ? "s" : ""}`);
        } else {
          lines.push(`Overdue by: ${diffHours} hour${diffHours !== 1 ? "s" : ""}`);
        }
      }
      break;
    }
    case "stale": {
      const diffMs = now.getTime() - row.updated_at.getTime();
      const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
      lines.push(`No update in: ${diffHours} hour${diffHours !== 1 ? "s" : ""}`);
      break;
    }
    case "approaching": {
      if (row.deadline !== null) {
        const diffMs = row.deadline.getTime() - now.getTime();
        const diffHours = Math.ceil(diffMs / (1000 * 60 * 60));
        lines.push(`Deadline in: ${diffHours} hour${diffHours !== 1 ? "s" : ""}`);
      }
      break;
    }
  }

  if (row.project_code !== null) {
    lines.push(`Project: ${row.project_code}`);
  }

  return lines.join("\n");
}

// ─── ProactiveWatcher ─────────────────────────────────────────────────────────

export class ProactiveWatcher {
  private _timer: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly db: Pool,
    private readonly telegram: TelegramAdapter,
    private readonly config: ProactiveWatcherConfig,
    private readonly logger: Logger,
    private readonly chatId: string,
    private readonly obligationStore?: ObligationStore,
  ) {}

  /**
   * Starts the watcher. Sets a recurring interval and fires one immediate scan.
   * No-op if already running.
   */
  start(): void {
    if (this._timer !== null) return;

    const intervalMs = this.config.intervalMinutes * 60_000;
    this._timer = setInterval(() => {
      void this.scan();
    }, intervalMs);

    // Fire immediately — don't wait for the first interval
    void this.scan();
  }

  /** Stops the watcher interval. */
  stop(): void {
    if (this._timer !== null) {
      clearInterval(this._timer);
      this._timer = null;
    }
  }

  /**
   * One full scan pass — queries for overdue, stale, and approaching obligations,
   * then sends up to `maxRemindersPerInterval` Telegram reminder cards.
   * Exposed for testing.
   */
  async scan(): Promise<void> {
    if (isQuietHours(new Date(), this.config.quietStart, this.config.quietEnd)) {
      this.logger.debug("quiet hours — skipping notification");
      return;
    }

    try {
      const candidates = await this._queryObligations();

      // Apply cap: oldest-first ordering is guaranteed by the queries (ORDER BY created_at ASC)
      const limited = candidates.slice(0, this.config.maxRemindersPerInterval);

      for (const { row, scanType } of limited) {
        await this._sendReminder(row, scanType);
      }
    } catch (err: unknown) {
      this.logger.error({ err }, "ProactiveWatcher scan failed");
    }
  }

  // ── Private helpers ─────────────────────────────────────────────────────────

  private async _queryObligations(): Promise<
    Array<{ row: ObligationRow; scanType: ScanType }>
  > {
    const { staleThresholdHours, approachingDeadlineHours } = this.config;

    const [overdueResult, staleResult, approachingResult] = await Promise.all([
      this.db.query<ObligationRow>(
        `SELECT * FROM obligations
         WHERE deadline IS NOT NULL
           AND deadline < NOW()
           AND status IN ('pending', 'in_progress')
         ORDER BY created_at ASC`,
      ),
      this.db.query<ObligationRow>(
        `SELECT * FROM obligations
         WHERE updated_at < NOW() - ($1 || ' hours')::interval
           AND status IN ('pending', 'in_progress')
         ORDER BY created_at ASC`,
        [String(staleThresholdHours)],
      ),
      this.db.query<ObligationRow>(
        `SELECT * FROM obligations
         WHERE deadline IS NOT NULL
           AND deadline BETWEEN NOW() AND NOW() + ($1 || ' hours')::interval
           AND status IN ('pending', 'in_progress')
         ORDER BY created_at ASC`,
        [String(approachingDeadlineHours)],
      ),
    ]);

    const results: Array<{ row: ObligationRow; scanType: ScanType }> = [
      ...overdueResult.rows.map((row) => ({ row, scanType: "overdue" as ScanType })),
      ...staleResult.rows.map((row) => ({ row, scanType: "stale" as ScanType })),
      ...approachingResult.rows.map((row) => ({ row, scanType: "approaching" as ScanType })),
    ];

    return results;
  }

  private async _sendReminder(row: ObligationRow, scanType: ScanType): Promise<void> {
    const text = formatReminderCard(row, scanType);
    const keyboard = watcherKeyboard(row.id);

    await this.telegram.sendMessage(this.chatId, text, {
      parseMode: "HTML",
      keyboard,
    });

    // Bridge: create obligation if store is available and the row's status
    // indicates it may not already be tracked as an active obligation
    if (this.obligationStore) {
      try {
        const existing = await this.obligationStore.getById(row.id);
        if (!existing) {
          const priority = scanType === "stale" ? 2 : 1; // approaching/overdue = P1
          await this.obligationStore.create({
            detectedAction: row.detected_action,
            owner: row.owner,
            status: ObligationStatus.Open,
            priority,
            projectCode: row.project_code,
            sourceChannel: "watcher",
            sourceMessage: `[${scanType}] Auto-created by proactive watcher`,
            deadline: row.deadline,
          });
          this.logger.info(
            { obligationId: row.id, scanType },
            "Watcher created obligation for untracked finding",
          );
        }
      } catch (err: unknown) {
        this.logger.warn(
          { err, obligationId: row.id },
          "Failed to bridge watcher finding to obligation",
        );
      }
    }

    this.logger.info(
      { obligationId: row.id, scanType },
      "Proactive watcher sent reminder",
    );
  }
}
