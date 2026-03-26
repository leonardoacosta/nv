import type { Pool } from "pg";
import type { Logger } from "pino";
import type { TelegramAdapter } from "../../channels/telegram.js";
import type { ProactiveWatcherConfig } from "./types.js";
import { watcherKeyboard } from "./callbacks.js";

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

// ─── isQuietHours ─────────────────────────────────────────────────────────────

/**
 * Returns true if `now` falls within the quiet hours window defined by
 * `config.quietStart` and `config.quietEnd` (HH:MM 24-hour strings).
 *
 * Handles the midnight wrap-around case (e.g. 22:00–07:00 spans midnight).
 * Uses local system time — no UTC conversion. The server TZ should match the
 * user's timezone for correct behaviour.
 */
export function isQuietHours(now: Date, config: ProactiveWatcherConfig): boolean {
  const [startHour = 0, startMin = 0] = config.quietStart.split(":").map(Number);
  const [endHour = 0, endMin = 0] = config.quietEnd.split(":").map(Number);

  const startMinutes = startHour * 60 + startMin;
  const endMinutes = endHour * 60 + endMin;
  const nowMinutes = now.getHours() * 60 + now.getMinutes();

  // When start === end, quiet hours are disabled (zero-length window)
  if (startMinutes === endMinutes) return false;

  if (startMinutes < endMinutes) {
    // Normal window: e.g. 09:00–17:00 — no midnight wrap
    return nowMinutes >= startMinutes && nowMinutes < endMinutes;
  } else {
    // Midnight wrap: e.g. 22:00–07:00
    // Active when: nowMinutes >= 22:00 OR nowMinutes < 07:00
    return nowMinutes >= startMinutes || nowMinutes < endMinutes;
  }
}

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
    if (isQuietHours(new Date(), this.config)) {
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

    this.logger.info(
      { obligationId: row.id, scanType },
      "Proactive watcher sent reminder",
    );
  }
}
