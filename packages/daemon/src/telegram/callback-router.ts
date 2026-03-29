/**
 * Declarative callback router for Telegram inline keyboard callbacks.
 *
 * Replaces the sequential if/startsWith chain in index.ts with a
 * Map-based prefix dispatch that extracts metadata once per message.
 */

import type { Message } from "../types.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface CallbackMeta {
  callbackQueryId: string;
  messageId: number;
  chatId: string;
}

/**
 * id: the portion of msg.text after the matched prefix.
 * meta: pre-extracted callbackQueryId, originalMessageId, chatId.
 * msg: the full original message for handlers that need it.
 */
export type CallbackHandler = (
  id: string,
  meta: CallbackMeta,
  msg: Message,
) => void;

// ─── CallbackRouter ───────────────────────────────────────────────────────────

export class CallbackRouter {
  private readonly handlers = new Map<string, CallbackHandler>();

  /**
   * Register a handler for a given prefix.
   * Prefixes are matched in insertion order — register more specific prefixes first.
   */
  register(prefix: string, handler: CallbackHandler): void {
    this.handlers.set(prefix, handler);
  }

  /**
   * Attempt to route msg to a registered handler.
   * Returns true if a handler was matched and invoked, false otherwise.
   *
   * Metadata is extracted once before prefix testing. All handlers receive the
   * same pre-extracted CallbackMeta.
   */
  route(msg: Message): boolean {
    const data = msg.text ?? "";

    // Extract metadata once
    const metadata = msg.metadata as
      | { callbackQueryId?: string; originalMessageId?: number }
      | undefined;

    const meta: CallbackMeta = {
      callbackQueryId: String(metadata?.callbackQueryId ?? ""),
      messageId: Number(metadata?.originalMessageId ?? 0),
      chatId: msg.chatId,
    };

    for (const [prefix, handler] of this.handlers) {
      if (data.startsWith(prefix)) {
        const id = data.slice(prefix.length);
        handler(id, meta, msg);
        return true;
      }
    }

    return false;
  }
}
