import type { Context } from "hono";
import type { Pool } from "pg";

// ─── Row shapes ───────────────────────────────────────────────────────────────

interface BriefingRow {
  id: string;
  generated_at: Date;
  content: string;
  sources_status: unknown;
  suggested_actions: unknown;
}

// ─── GET /api/briefing ────────────────────────────────────────────────────────

/**
 * Returns the most recently generated briefing.
 * Responds 404 when no briefings have been generated yet.
 */
export async function handleBriefingGet(c: Context, getPool: () => Pool): Promise<Response> {
  const pool = getPool();

  const result = await pool.query<BriefingRow>(
    `SELECT id, generated_at, content, sources_status, suggested_actions
     FROM briefings
     ORDER BY generated_at DESC
     LIMIT 1`,
  );

  const row = result.rows[0];
  if (!row) {
    return c.json({ error: "no briefing available" }, 404);
  }

  return c.json({
    id: row.id,
    generated_at: row.generated_at.toISOString(),
    content: row.content,
    sources_status: row.sources_status,
    suggested_actions: row.suggested_actions,
  });
}

// ─── GET /api/briefing/history ────────────────────────────────────────────────

/**
 * Returns briefing history, newest first.
 *
 * Query params:
 *   limit — integer (default: 10, max: 30)
 *
 * Response: { entries: [...] } — empty array is a valid response.
 */
export async function handleBriefingHistory(c: Context, getPool: () => Pool): Promise<Response> {
  const rawLimit = parseInt(c.req.query("limit") ?? "10", 10);
  const limit = Math.min(30, Math.max(1, isNaN(rawLimit) ? 10 : rawLimit));

  const pool = getPool();

  const result = await pool.query<BriefingRow>(
    `SELECT id, generated_at, content, sources_status, suggested_actions
     FROM briefings
     ORDER BY generated_at DESC
     LIMIT $1`,
    [limit],
  );

  const entries = result.rows.map((row) => ({
    id: row.id,
    generated_at: row.generated_at.toISOString(),
    content: row.content,
    sources_status: row.sources_status,
    suggested_actions: row.suggested_actions,
  }));

  return c.json({ entries });
}
