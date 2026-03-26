import type { Pool } from "pg";
import type { Message } from "../types.js";

interface MessageRow {
  id: string;
  channel: string;
  sender: string | null;
  content: string;
  metadata: Record<string, unknown> | null;
  created_at: Date;
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
   */
  async loadHistory(channelId: string, limit: number): Promise<Message[]> {
    const result = await this.pool.query<MessageRow>(
      `SELECT id, channel, sender, content, metadata, created_at
       FROM messages
       WHERE channel = $1
       ORDER BY created_at DESC
       LIMIT $2`,
      [channelId, limit],
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
    const client = await this.pool.connect();
    try {
      await client.query("BEGIN");

      await client.query(
        `INSERT INTO messages (channel, sender, content, metadata)
         VALUES ($1, $2, $3, $4)`,
        [
          channelId,
          userMsg.senderId,
          userMsg.content,
          userMsg.metadata ? JSON.stringify(userMsg.metadata) : null,
        ],
      );

      await client.query(
        `INSERT INTO messages (channel, sender, content, metadata)
         VALUES ($1, $2, $3, $4)`,
        [
          channelId,
          "nova",
          assistantMsg.content,
          assistantMsg.metadata ? JSON.stringify(assistantMsg.metadata) : null,
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
