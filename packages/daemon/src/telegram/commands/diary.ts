import { getEntriesByDate } from "../../features/diary/index.js";
import type { DiaryEntryItem } from "../../features/diary/index.js";

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const TELEGRAM_MAX_CHARS = 4000;
const ENTRIES_LIMIT = 10;

function formatEntry(entry: DiaryEntryItem): string {
  const time = new Date(entry.time).toTimeString().slice(0, 8); // HH:MM:SS
  const tools =
    entry.tools_called.length > 0 ? entry.tools_called.join(", ") : "none";
  const slug =
    entry.slug.length > 60 ? `${entry.slug.slice(0, 57)}...` : entry.slug;

  return [
    `[${time}] ${entry.trigger_type} via ${entry.channel_source}`,
    `  from: ${entry.trigger_source}`,
    `  slug: ${slug}`,
    `  tools: ${tools}`,
    `  tokens: ${entry.tokens_in}in / ${entry.tokens_out}out`,
    `  latency: ${entry.response_latency_ms}ms`,
  ].join("\n");
}

/**
 * Build the Telegram message text for /diary output.
 * Truncated to TELEGRAM_MAX_CHARS if necessary.
 */
export async function buildDiaryReply(dateArg?: string): Promise<string> {
  let dateStr: string;

  if (dateArg && DATE_RE.test(dateArg)) {
    dateStr = dateArg;
  } else {
    dateStr = new Date().toISOString().slice(0, 10);
  }

  let entries = await getEntriesByDate(dateStr, ENTRIES_LIMIT);

  // If today has no entries, fall back to yesterday
  const isToday = dateStr === new Date().toISOString().slice(0, 10);
  if (entries.length === 0 && isToday && !dateArg) {
    const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
    entries = await getEntriesByDate(yesterday, ENTRIES_LIMIT);
    dateStr = yesterday;
  }

  if (entries.length === 0) {
    return `No diary entries found for ${dateStr}.`;
  }

  const header = `Diary — ${dateStr} (${entries.length} entries)\n${"─".repeat(32)}\n`;
  const body = entries.map(formatEntry).join("\n\n");
  const full = header + body;

  if (full.length <= TELEGRAM_MAX_CHARS) {
    return full;
  }

  return full.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
