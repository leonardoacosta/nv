import { randomUUID } from "node:crypto";
import type { Pool } from "pg";
import { ObligationStatus, type ObligationRecord, type CreateObligationInput } from "./types.js";

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

function rowToRecord(row: ObligationRow): ObligationRecord {
  return {
    id: row.id,
    detectedAction: row.detected_action,
    owner: row.owner,
    status: row.status as ObligationStatus,
    priority: row.priority,
    projectCode: row.project_code,
    sourceChannel: row.source_channel,
    sourceMessage: row.source_message,
    deadline: row.deadline,
    lastAttemptAt: row.last_attempt_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

// ─── ObligationStore ──────────────────────────────────────────────────────────

export class ObligationStore {
  constructor(private readonly pool: Pool) {}

  async create(input: CreateObligationInput): Promise<ObligationRecord> {
    const id = randomUUID();
    const now = new Date();

    const result = await this.pool.query<ObligationRow>(
      `INSERT INTO obligations
         (id, detected_action, owner, status, priority, project_code,
          source_channel, source_message, deadline, last_attempt_at,
          created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)
       RETURNING *`,
      [
        id,
        input.detectedAction,
        input.owner,
        input.status,
        input.priority,
        input.projectCode,
        input.sourceChannel,
        input.sourceMessage,
        input.deadline,
        now,
        now,
      ],
    );

    const row = result.rows[0];
    if (!row) {
      throw new Error("INSERT did not return a row");
    }
    return rowToRecord(row);
  }

  async getById(id: string): Promise<ObligationRecord | null> {
    const result = await this.pool.query<ObligationRow>(
      "SELECT * FROM obligations WHERE id = $1",
      [id],
    );

    const row = result.rows[0];
    return row ? rowToRecord(row) : null;
  }

  async listByStatus(status: ObligationStatus): Promise<ObligationRecord[]> {
    const result = await this.pool.query<ObligationRow>(
      "SELECT * FROM obligations WHERE status = $1 ORDER BY created_at ASC",
      [status],
    );

    return result.rows.map(rowToRecord);
  }

  /**
   * Returns obligations ready for autonomous execution:
   * - owner = "nova"
   * - status IN ("open", "in_progress")
   * - last_attempt_at IS NULL OR last_attempt_at < now() - cooldownHours
   * - ordered by priority ASC, then created_at ASC (P1 first, oldest first within priority)
   */
  async listReadyForExecution(cooldownHours = 2): Promise<ObligationRecord[]> {
    const result = await this.pool.query<ObligationRow>(
      `SELECT * FROM obligations
       WHERE owner = 'nova'
         AND status IN ('open', 'in_progress')
         AND (
           last_attempt_at IS NULL
           OR last_attempt_at < NOW() - ($1 || ' hours')::interval
         )
       ORDER BY priority ASC, created_at ASC`,
      [String(cooldownHours)],
    );

    return result.rows.map(rowToRecord);
  }

  async updateStatus(id: string, status: ObligationStatus): Promise<void> {
    await this.pool.query(
      "UPDATE obligations SET status = $1, updated_at = $2 WHERE id = $3",
      [status, new Date(), id],
    );
  }

  async updateLastAttemptAt(id: string, timestamp: Date): Promise<void> {
    await this.pool.query(
      "UPDATE obligations SET last_attempt_at = $1, updated_at = $2 WHERE id = $3",
      [timestamp, new Date(), id],
    );
  }

  /**
   * Appends a timestamped note to the source_message field (used as a notes
   * accumulator). If source_message is null, sets it to the note directly.
   */
  async appendNote(id: string, note: string): Promise<void> {
    const current = await this.getById(id);
    if (!current) {
      throw new Error(`Obligation not found: ${id}`);
    }

    const timestamp = new Date().toISOString();
    const entry = `[${timestamp}] ${note}`;
    const updated =
      current.sourceMessage !== null
        ? `${current.sourceMessage}\n${entry}`
        : entry;

    await this.pool.query(
      "UPDATE obligations SET source_message = $1, updated_at = $2 WHERE id = $3",
      [updated, new Date(), id],
    );
  }
}
