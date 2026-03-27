import { ObligationStatus } from "./types.js";
import type { ObligationStore } from "./store.js";

// ─── Callback prefix constants ────────────────────────────────────────────────

export const OBLIGATION_CONFIRM_PREFIX = "obligation_confirm:";
export const OBLIGATION_REOPEN_PREFIX = "obligation_reopen:";
export const OBLIGATION_ESCALATION_RETRY_PREFIX = "obligation_esc_retry:";
export const OBLIGATION_ESCALATION_DISMISS_PREFIX = "obligation_esc_dismiss:";
export const OBLIGATION_ESCALATION_TAKEOVER_PREFIX = "obligation_esc_take:";

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

// ─── Escalation Handlers ─────────────────────────────────────────────────────

/**
 * Handles the "Retry" button on an escalated obligation.
 * Resets attempt_count to 0 and sets status back to open.
 */
export async function handleEscalationRetry(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  chatId: number | string,
  messageId: number,
): Promise<void> {
  const obligation = await store.getById(id);
  if (!obligation) return;

  if (obligation.status !== ObligationStatus.Escalated) return;

  await store.resetAttemptCount(id);
  await store.updateStatus(id, ObligationStatus.Open);
  await telegram.editMessage(chatId, messageId, "Retry queued — attempt count reset.");
}

/**
 * Handles the "Dismiss" button on an escalated obligation.
 * Sets status to dismissed.
 */
export async function handleEscalationDismiss(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  chatId: number | string,
  messageId: number,
): Promise<void> {
  const obligation = await store.getById(id);
  if (!obligation) return;

  if (obligation.status !== ObligationStatus.Escalated) return;

  await store.updateStatus(id, ObligationStatus.Dismissed);
  await telegram.editMessage(chatId, messageId, "Obligation dismissed.");
}

/**
 * Handles the "Take Over" button on an escalated obligation.
 * Changes owner to "leo" and status to open, resets attempt count.
 */
export async function handleEscalationTakeover(
  id: string,
  store: ObligationStore,
  telegram: TelegramSender,
  chatId: number | string,
  messageId: number,
): Promise<void> {
  const obligation = await store.getById(id);
  if (!obligation) return;

  if (obligation.status !== ObligationStatus.Escalated) return;

  await store.resetAttemptCount(id);
  await store.updateOwner(id, "leo");
  await store.updateStatus(id, ObligationStatus.Open);
  await telegram.editMessage(chatId, messageId, "Transferred to Leo.");
}
