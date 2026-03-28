import type { Pool } from "pg";
import type { Message } from "../types.js";

interface MessageRow {
  id: string;
  channel: string;
  sender: string | null;
  content: string;
  metadata: Record<string, unknown> | null;
  created_at: Date;
  thread_id: string | null;
  reply_to_message_id: number | null;
}

function rowToMessage(row: MessageRow): Message {
  return {
    id: row.id,
    channel: row.channel as Message["channel"],
    chatId: row.channel,
    text: row.content,
    content: row.content,
    type: "text",
    from: {
      id: row.sender ?? "unknown",
      firstName: row.sender ?? "unknown",
    },
    senderId: row.sender ?? "unknown",
    senderName: row.sender ?? "unknown",
    timestamp: row.created_at,
    receivedAt: row.created_at,
    metadata: row.metadata ?? {},
    ...(row.thread_id != null && { threadId: row.thread_id }),
    ...(row.reply_to_message_id != null && {
      replyToMessageId: row.reply_to_message_id,
    }),
  };
}

export class ConversationManager {
  private readonly pool: Pool;

  constructor(pool: Pool) {
    this.pool = pool;
  }

  /**
   * Load the most recent `limit` messages for a channel, returned in
   * chronological order (oldest first).
   *
   * When `threadId` is provided only messages belonging to that thread are
   * returned; omitting it preserves the original behaviour (all messages for
   * the channel).
   */
  async loadHistory(
    channelId: string,
    limit: number,
    threadId?: string,
  ): Promise<Message[]> {
    const params: (string | number)[] = [channelId, limit];
    const threadClause =
      threadId != null
        ? `AND thread_id = $${params.push(threadId)}`
        : "";

    const result = await this.pool.query<MessageRow>(
      `SELECT id, channel, sender, content, metadata, created_at,
              thread_id, reply_to_message_id
       FROM messages
       WHERE channel = $1
       ${threadClause}
       ORDER BY created_at DESC
       LIMIT $2`,
      params,
    );

    // Reverse so messages are in chronological order
    return result.rows.reverse().map(rowToMessage);
  }

  /**
   * Insert both the user message and assistant reply in a single transaction.
   * The assistant message's sender is normalised to "nova".
   */
  async saveExchange(
    channelId: string,
    userMsg: Message,
    assistantMsg: Message,
  ): Promise<void> {
    const threadId = userMsg.threadId ?? null;
    const replyToMessageId = userMsg.replyToMessageId ?? null;

    const client = await this.pool.connect();
    try {
      await client.query("BEGIN");

      await client.query(
        `INSERT INTO messages
           (channel, sender, content, metadata, thread_id, reply_to_message_id)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [
          channelId,
          userMsg.senderId,
          userMsg.content,
          userMsg.metadata ? JSON.stringify(userMsg.metadata) : null,
          threadId,
          replyToMessageId,
        ],
      );

      await client.query(
        `INSERT INTO messages
           (channel, sender, content, metadata, thread_id, reply_to_message_id)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [
          channelId,
          "nova",
          assistantMsg.content,
          assistantMsg.metadata ? JSON.stringify(assistantMsg.metadata) : null,
          threadId,
          null,
        ],
      );

      await client.query("COMMIT");
    } catch (err: unknown) {
      await client.query("ROLLBACK");
      throw err;
    } finally {
      client.release();
    }
  }
}
