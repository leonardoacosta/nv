import type { Pool } from "pg";
import type TelegramBot from "node-telegram-bot-api";
import { buildKeyboard } from "../../channels/telegram.js";
import type { TelegramAdapter } from "../../channels/telegram.js";
import { createLogger } from "../../logger.js";

const log = createLogger("watcher-callbacks");

// ─── Callback prefix constants ────────────────────────────────────────────────

export const WATCHER_DONE_PREFIX = "watcher:done:";
export const WATCHER_SNOOZE_PREFIX = "watcher:snooze:";
export const WATCHER_DISMISS_PREFIX = "watcher:dismiss:";

// ─── watcherKeyboard ──────────────────────────────────────────────────────────

/**
 * Builds the inline keyboard for a watcher reminder card.
 *
 * Buttons:
 *   [Mark Done]   → watcher:done:{id}
 *   [Snooze 24h]  → watcher:snooze:{id}
 *   [Dismiss]     → watcher:dismiss:{id}
 */
export function watcherKeyboard(
  obligationId: string,
): TelegramBot.InlineKeyboardMarkup {
  return buildKeyboard([
    [
      { text: "Mark Done", callbackData: `${WATCHER_DONE_PREFIX}${obligationId}` },
      { text: "Snooze 24h", callbackData: `${WATCHER_SNOOZE_PREFIX}${obligationId}` },
      { text: "Dismiss", callbackData: `${WATCHER_DISMISS_PREFIX}${obligationId}` },
    ],
  ]);
}

// ─── handleWatcherCallback ────────────────────────────────────────────────────

/**
 * Handles inline keyboard callbacks from watcher reminder cards.
 *
 * Actions:
 *   watcher:done:{id}    → sets status = 'done', updated_at = NOW()
 *   watcher:snooze:{id}  → advances updated_at by 24h (resets stale clock without status change)
 *   watcher:dismiss:{id} → sets status = 'cancelled', updated_at = NOW()
 *
 * Note on snooze: setting updated_at to now + 24h is a deliberate data trick.
 * Obligations with updated_at > NOW() will not match the stale query
 * (`updated_at < NOW() - threshold`) for the next 24 hours, effectively
 * snoozing the reminder without changing the obligation status.
 *
 * The callbackQueryId must be answered FIRST (before any DB operations) to
 * dismiss the Telegram spinner before the 60-second expiry.
 */
export async function handleWatcherCallback(
  data: string,
  db: Pool,
  telegram: TelegramAdapter,
  messageId: number,
  chatId: string,
  callbackQueryId: string,
): Promise<void> {
  // Answer immediately to avoid Telegram spinner expiry (60s limit)
  await telegram.answerCallbackQuery(callbackQueryId);

  let obligationId: string;
  let confirmationText: string;

  if (data.startsWith(WATCHER_DONE_PREFIX)) {
    obligationId = data.slice(WATCHER_DONE_PREFIX.length);
    await db.query(
      "UPDATE obligations SET status = 'done', updated_at = $1 WHERE id = $2",
      [new Date(), obligationId],
    );
    confirmationText = "Obligation marked done.";
    log.info({ obligationId }, "Watcher: obligation marked done");
  } else if (data.startsWith(WATCHER_SNOOZE_PREFIX)) {
    obligationId = data.slice(WATCHER_SNOOZE_PREFIX.length);
    // Advance updated_at 24h into the future — natural stale-query exclusion
    const snoozeUntil = new Date(Date.now() + 24 * 60 * 60 * 1000);
    await db.query(
      "UPDATE obligations SET updated_at = $1 WHERE id = $2",
      [snoozeUntil, obligationId],
    );
    confirmationText = "Obligation snoozed for 24 hours.";
    log.info({ obligationId, snoozeUntil }, "Watcher: obligation snoozed");
  } else if (data.startsWith(WATCHER_DISMISS_PREFIX)) {
    obligationId = data.slice(WATCHER_DISMISS_PREFIX.length);
    await db.query(
      "UPDATE obligations SET status = 'cancelled', updated_at = $1 WHERE id = $2",
      [new Date(), obligationId],
    );
    confirmationText = "Obligation dismissed.";
    log.info({ obligationId }, "Watcher: obligation dismissed");
  } else {
    // Unknown prefix — ignore silently
    log.debug({ data }, "Watcher: unrecognised callback prefix — ignoring");
    return;
  }

  // Edit the original message to show confirmation and remove the keyboard
  await telegram.editMessage(chatId, messageId, confirmationText);
}
