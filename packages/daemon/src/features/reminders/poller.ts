import type { Pool } from "pg";
import type { Logger } from "pino";
import type { TelegramAdapter } from "../../channels/telegram.js";
import { reminderKeyboard } from "../../channels/telegram.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ReminderPollerDeps {
  pool: Pool;
  logger: Logger;
  telegram: TelegramAdapter;
  telegramChatId: string;
}

interface ReminderRow {
  id: string;
  message: string;
  due_at: Date;
  channel: string;
  obligation_id: string | null;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const POLL_INTERVAL_MS = 15_000; // 15 seconds — fast enough for reminders
const BATCH_LIMIT = 10; // Process up to 10 reminders per tick

// ─── startReminderPoller ──────────────────────────────────────────────────────

/**
 * Polls the `reminders` table for due reminders and delivers them via Telegram.
 *
 * - Polls every 15 seconds.
 * - Queries for reminders where `due_at <= now()`, `delivered_at IS NULL`, and `cancelled = false`.
 * - Sends each reminder to Telegram with Done/Snooze inline keyboard.
 * - Updates `delivered_at` after successful delivery.
 * - Processes up to 10 reminders per tick to avoid flooding.
 *
 * Returns a cleanup function that clears the interval.
 */
export function startReminderPoller(deps: ReminderPollerDeps): () => void {
  const { pool, logger, telegram, telegramChatId } = deps;
  let running = false;

  const interval = setInterval(() => {
    if (running) return; // Prevent overlapping ticks
    running = true;

    void pollAndDeliver().finally(() => {
      running = false;
    });
  }, POLL_INTERVAL_MS);

  async function pollAndDeliver(): Promise<void> {
    try {
      const result = await pool.query<ReminderRow>(
        `SELECT id, message, due_at, channel, obligation_id
         FROM reminders
         WHERE due_at <= now()
           AND delivered_at IS NULL
           AND cancelled = false
         ORDER BY due_at ASC
         LIMIT $1`,
        [BATCH_LIMIT],
      );

      if (result.rows.length === 0) return;

      logger.info(
        { count: result.rows.length },
        "Reminder poller: delivering due reminders",
      );

      for (const row of result.rows) {
        try {
          const text = `⏰ *Reminder*\n\n${row.message}`;

          await telegram.sendMessage(telegramChatId, text, {
            parseMode: "Markdown",
            keyboard: reminderKeyboard(row.id),
          });

          // Mark as delivered
          await pool.query(
            `UPDATE reminders SET delivered_at = now() WHERE id = $1`,
            [row.id],
          );

          logger.info(
            { reminderId: row.id, message: row.message.slice(0, 60) },
            "Reminder delivered",
          );
        } catch (err: unknown) {
          logger.error(
            { err, reminderId: row.id },
            "Failed to deliver reminder — will retry next tick",
          );
          // Don't update delivered_at — it will be retried on the next poll
        }
      }
    } catch (err: unknown) {
      logger.error({ err }, "Reminder poller: query failed");
    }
  }

  return () => {
    clearInterval(interval);
  };
}

// ─── Callback handlers ───────────────────────────────────────────────────────

/**
 * Handle `reminder:done:<id>` callback — marks the reminder as acknowledged.
 * (delivered_at is already set; this is just UX confirmation.)
 */
export async function handleReminderDone(
  reminderId: string,
  pool: Pool,
  telegram: TelegramAdapter,
  chatId: string | number,
  messageId: number,
  callbackQueryId: string,
): Promise<void> {
  try {
    await telegram.answerCallbackQuery(callbackQueryId, "✅ Done");

    // Edit the message to remove the keyboard and mark as done
    const result = await pool.query<{ message: string }>(
      `SELECT message FROM reminders WHERE id = $1`,
      [reminderId],
    );
    const msg = result.rows[0]?.message ?? "Reminder";
    await telegram.editMessage(chatId, messageId, `✅ ~${msg}~`);
  } catch {
    // Best-effort — don't throw
  }
}

/**
 * Handle `reminder:snooze:<duration>:<id>` callback — reschedules the reminder.
 */
export async function handleReminderSnooze(
  reminderId: string,
  duration: string,
  pool: Pool,
  telegram: TelegramAdapter,
  chatId: string | number,
  messageId: number,
  callbackQueryId: string,
): Promise<void> {
  try {
    // Calculate new due_at
    let intervalExpr: string;
    let humanLabel: string;

    if (duration === "1h") {
      intervalExpr = "interval '1 hour'";
      humanLabel = "1 hour";
    } else if (duration === "tomorrow") {
      // Tomorrow at 9am local
      intervalExpr = "(date_trunc('day', now()) + interval '1 day' + interval '9 hours') - now()";
      humanLabel = "tomorrow morning";
    } else {
      intervalExpr = "interval '1 hour'"; // Fallback
      humanLabel = "1 hour";
    }

    // Reset delivered_at and bump due_at
    await pool.query(
      `UPDATE reminders
       SET due_at = now() + ${intervalExpr},
           delivered_at = NULL
       WHERE id = $1`,
      [reminderId],
    );

    await telegram.answerCallbackQuery(callbackQueryId, `⏰ Snoozed ${humanLabel}`);

    // Edit the message to reflect snooze
    const result = await pool.query<{ message: string }>(
      `SELECT message FROM reminders WHERE id = $1`,
      [reminderId],
    );
    const msg = result.rows[0]?.message ?? "Reminder";
    await telegram.editMessage(
      chatId,
      messageId,
      `💤 Snoozed: ${msg}\n\n_Will remind again in ${humanLabel}_`,
    );
  } catch {
    // Best-effort
  }
}
