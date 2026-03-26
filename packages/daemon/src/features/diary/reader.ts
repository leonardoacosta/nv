import { db, diary } from "@nova/db";
import type { DiaryEntry } from "@nova/db";
import { and, gte, lt, lte, desc } from "drizzle-orm";

/**
 * DiaryEntryItem — matches `DiaryEntryItem` in `apps/dashboard/types/api.ts`.
 * This is the canonical shape returned by the HTTP API and Telegram command.
 */
export interface DiaryEntryItem {
  time: string;
  trigger_type: string;
  trigger_source: string;
  channel_source: string;
  slug: string;
  tools_called: string[];
  result_summary: string;
  response_latency_ms: number;
  tokens_in: number;
  tokens_out: number;
}

function rowToItem(row: DiaryEntry): DiaryEntryItem {
  return {
    time: row.createdAt.toISOString(),
    trigger_type: row.triggerType,
    trigger_source: row.triggerSource,
    channel_source: row.channel,
    slug: row.slug,
    tools_called: Array.isArray(row.toolsUsed)
      ? (row.toolsUsed as string[])
      : [],
    result_summary: row.content,
    response_latency_ms: row.responseLatencyMs ?? 0,
    tokens_in: row.tokensIn ?? 0,
    tokens_out: row.tokensOut ?? 0,
  };
}

/**
 * Returns diary entries for a given YYYY-MM-DD date, ordered newest-first.
 * The date is interpreted as UTC midnight boundaries.
 */
export async function getEntriesByDate(
  date: string,
  limit = 50,
): Promise<DiaryEntryItem[]> {
  const startOfDay = new Date(`${date}T00:00:00.000Z`);
  const startOfNextDay = new Date(startOfDay.getTime() + 86_400_000);

  const rows = await db
    .select()
    .from(diary)
    .where(and(gte(diary.createdAt, startOfDay), lt(diary.createdAt, startOfNextDay)))
    .orderBy(desc(diary.createdAt))
    .limit(limit);

  return rows.map(rowToItem);
}

/**
 * Returns diary entries for an inclusive date range [from, to] (YYYY-MM-DD).
 * Ordered newest-first.
 */
export async function getEntriesByDateRange(
  from: string,
  to: string,
): Promise<DiaryEntryItem[]> {
  const startOfFrom = new Date(`${from}T00:00:00.000Z`);
  const endOfTo = new Date(`${to}T23:59:59.999Z`);

  const rows = await db
    .select()
    .from(diary)
    .where(and(gte(diary.createdAt, startOfFrom), lte(diary.createdAt, endOfTo)))
    .orderBy(desc(diary.createdAt));

  return rows.map(rowToItem);
}
