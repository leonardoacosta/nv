import { randomUUID } from "node:crypto";
import type { Pool } from "pg";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface MemoryRecord {
  id: string;
  topic: string;
  content: string;
  updated_at: Date;
}

// ─── Row shape returned by pg (snake_case columns) ───────────────────────────

interface MemoryRow {
  id: string;
  topic: string;
  content: string;
  updated_at: Date;
}

function rowToRecord(row: MemoryRow): MemoryRecord {
  return {
    id: row.id,
    topic: row.topic,
    content: row.content,
    updated_at: row.updated_at,
  };
}

// ─── MemoryStore ──────────────────────────────────────────────────────────────

export class MemoryStore {
  constructor(private readonly pool: Pool) {}

  /**
   * Inserts a new memory row or updates content + updated_at if topic already exists.
   */
  async upsert(topic: string, content: string): Promise<MemoryRecord> {
    const id = randomUUID();
    const now = new Date();

    const result = await this.pool.query<MemoryRow>(
      `INSERT INTO memory (id, topic, content, updated_at)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT (topic) DO UPDATE
         SET content    = EXCLUDED.content,
             updated_at = EXCLUDED.updated_at
       RETURNING *`,
      [id, topic, content, now],
    );

    const row = result.rows[0];
    if (!row) {
      throw new Error("UPSERT did not return a row");
    }
    return rowToRecord(row);
  }

  /**
   * Returns the memory row for the given topic, or null if not found.
   */
  async get(topic: string): Promise<MemoryRecord | null> {
    const result = await this.pool.query<MemoryRow>(
      "SELECT * FROM memory WHERE topic = $1",
      [topic],
    );

    const row = result.rows[0];
    return row ? rowToRecord(row) : null;
  }

  /**
   * Returns all topics with their updated_at timestamps (no content).
   */
  async list(): Promise<{ topic: string; updated_at: Date }[]> {
    const result = await this.pool.query<{ topic: string; updated_at: Date }>(
      "SELECT topic, updated_at FROM memory ORDER BY updated_at DESC",
    );
    return result.rows;
  }

  /**
   * Removes a memory row by topic. No-op if not found.
   */
  async delete(topic: string): Promise<void> {
    await this.pool.query("DELETE FROM memory WHERE topic = $1", [topic]);
  }
}
