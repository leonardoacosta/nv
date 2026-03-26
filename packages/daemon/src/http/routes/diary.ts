import type { Context } from "hono";
import { getEntriesByDate } from "../../features/diary/index.js";

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * GET /api/diary
 *
 * Query params:
 *   date  — YYYY-MM-DD (default: today UTC)
 *   limit — integer (default: 50, max: 200)
 *
 * Response: { date, entries: DiaryEntryItem[], total }
 */
export async function handleDiaryGet(c: Context): Promise<Response> {
  const dateParam = c.req.query("date");
  const today = new Date().toISOString().slice(0, 10);
  const dateStr = dateParam ?? today;

  if (!DATE_RE.test(dateStr)) {
    return c.json({ error: "Invalid date format — expected YYYY-MM-DD" }, 400);
  }

  const rawLimit = parseInt(c.req.query("limit") ?? "50", 10);
  const limit = Math.min(200, Math.max(1, isNaN(rawLimit) ? 50 : rawLimit));

  const entries = await getEntriesByDate(dateStr, limit);

  return c.json({
    date: dateStr,
    entries,
    total: entries.length,
  });
}
