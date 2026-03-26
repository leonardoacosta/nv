import { ObligationStatus } from "./types.js";
import type { ObligationStore } from "./store.js";

// ─── Callback prefix constants ────────────────────────────────────────────────

export const OBLIGATION_CONFIRM_PREFIX = "obligation_confirm:";
export const OBLIGATION_REOPEN_PREFIX = "obligation_reopen:";

// ─── TelegramSender interface ─────────────────────────────────────────────────

export interface TelegramSender {
  editMessage(
    chatId: number | string,
    messageId: number,
    text: string,
  ): Promise<void>;
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/**
 * Handles the "Confirm Done" inline button press.
 * Transitions proposed_done -> done and edits the Telegram message.
 */
export async function handleObligationConfirm(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  chatId: number | string,
  messageId: number,
): Promise<void> {
  const obligation = await store.getById(id);
  if (!obligation) {
    return;
  }

  if (obligation.status !== ObligationStatus.ProposedDone) {
    // Already transitioned — ignore duplicate callbacks
    return;
  }

  await store.updateStatus(id, ObligationStatus.Done);
  await telegram.editMessage(chatId, messageId, "Obligation confirmed.");
}

/**
 * Handles the "Reopen" inline button press.
 * Transitions proposed_done -> open and edits the Telegram message.
 */
export async function handleObligationReopen(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  chatId: number | string,
  messageId: number,
): Promise<void> {
  const obligation = await store.getById(id);
  if (!obligation) {
    return;
  }

  if (obligation.status !== ObligationStatus.ProposedDone) {
    // Already transitioned — ignore duplicate callbacks
    return;
  }

  await store.updateStatus(id, ObligationStatus.Open);
  await telegram.editMessage(chatId, messageId, "Reopened — Nova will retry.");
}
