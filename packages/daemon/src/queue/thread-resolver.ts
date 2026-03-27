import type { Pool } from "pg";
import { createLogger } from "../logger.js";

const logger = createLogger("thread-resolver");

interface MessageChainRow {
  reply_to_message_id: number | null;
}

/**
 * Resolves the root thread ID for a given Telegram message.
 *
 * A thread ID is a string of the form `"chatId:rootTelegramMessageId"` where
 * `rootTelegramMessageId` is the Telegram messageId of the message that
 * started the reply chain (i.e. the first message with no `reply_to_message_id`).
 *
 * Resolution strategy:
 * 1. If the message has no `replyToMessageId`, it IS the thread root.
 * 2. Otherwise, check the in-memory cache for the parent messageId.
 * 3. On cache miss, walk the `messages` table following `reply_to_message_id`
 *    until reaching a row where `reply_to_message_id IS NULL`.
 *
 * Cache key format: `"chatId:telegramMessageId"` → root telegramMessageId (number).
 */
export class ThreadResolver {
  private readonly pool: Pool;
  /** Maps "chatId:telegramMessageId" → root telegramMessageId */
  private readonly cache = new Map<string, number>();

  constructor(pool: Pool) {
    this.pool = pool;
  }

  /**
   * Resolve the thread ID for a message.
   *
   * @param chatId           - Telegram chat ID (stored as `channel` in the DB)
   * @param messageId        - Telegram messageId of the current message
   * @param replyToMessageId - Telegram messageId this message is quoting, if any
   * @returns Thread ID string in the form `"chatId:rootMessageId"`
   */
  async resolve(
    chatId: string,
    messageId: number,
    replyToMessageId?: number,
  ): Promise<string> {
    // No quote → this message is itself the thread root
    if (replyToMessageId === undefined) {
      this.cache.set(`${chatId}:${messageId}`, messageId);
      return `${chatId}:${messageId}`;
    }

    // Check cache for the parent first
    const parentCacheKey = `${chatId}:${replyToMessageId}`;
    const cached = this.cache.get(parentCacheKey);
    if (cached !== undefined) {
      this.cache.set(`${chatId}:${messageId}`, cached);
      logger.debug(
        { chatId, messageId, replyToMessageId, root: cached },
        "thread resolved from cache",
      );
      return `${chatId}:${cached}`;
    }

    // Cache miss → walk the DB chain
    const root = await this.walkChain(chatId, replyToMessageId);
    this.cache.set(parentCacheKey, root);
    this.cache.set(`${chatId}:${messageId}`, root);
    logger.debug(
      { chatId, messageId, replyToMessageId, root },
      "thread resolved from DB walk",
    );
    return `${chatId}:${root}`;
  }

  /**
   * Walk the `messages` table following the `reply_to_message_id` chain for
   * messages in the given chat (`channel = 'telegram'` and the `chatId` stored
   * in `metadata->>'chatId'`), starting from `startMessageId`.
   *
   * Each hop: look up the row whose `metadata->>'messageId' = startMessageId`
   * and read its `reply_to_message_id`. Repeat until reaching a row where
   * `reply_to_message_id IS NULL` — that row's Telegram messageId is the root.
   *
   * Falls back to returning `startMessageId` if the message is not found in
   * the DB (e.g. the row predates the `reply_to_message_id` column migration).
   */
  private async walkChain(chatId: string, startMessageId: number): Promise<number> {
    let current = startMessageId;
    // Guard against malformed data producing an infinite loop
    const MAX_HOPS = 100;

    for (let hop = 0; hop < MAX_HOPS; hop++) {
      const result = await this.pool.query<MessageChainRow>(
        `SELECT reply_to_message_id
         FROM messages
         WHERE channel = 'telegram'
           AND metadata->>'chatId' = $1
           AND metadata->>'messageId' = $2
         LIMIT 1`,
        [chatId, String(current)],
      );

      const row = result.rows[0];

      if (!row) {
        // Row not found — treat current as root (pre-migration message or gap)
        logger.warn(
          { chatId, messageId: current, startMessageId },
          "thread-resolver: message not found in DB, treating as root",
        );
        return current;
      }

      if (row.reply_to_message_id === null) {
        // Found the root
        return current;
      }

      // Check cache before the next hop
      const nextCacheKey = `${chatId}:${row.reply_to_message_id}`;
      const cachedRoot = this.cache.get(nextCacheKey);
      if (cachedRoot !== undefined) {
        return cachedRoot;
      }

      current = row.reply_to_message_id;
    }

    logger.error(
      { chatId, startMessageId, hops: MAX_HOPS },
      "thread-resolver: MAX_HOPS exceeded, returning start as root",
    );
    return startMessageId;
  }
}
