import type TelegramBot from "node-telegram-bot-api";
import { buildKeyboard } from "../../channels/telegram.js";
import type { DigestItem, Priority } from "./classify.js";

// ─── Constants ────────────────────────────────────────────────────────────────

const TELEGRAM_MAX_LEN = 4096;

const SOURCE_ICONS: Record<string, string> = {
  email: "[Mail]",
  teams: "[Teams]",
  calendar: "[Cal]",
  pim: "[PIM]",
  ado: "[ADO]",
  obligation: "[Ob]",
};

const PRIORITY_HEADERS: Record<Priority, string> = {
  P0: "*URGENT*",
  P1: "*Action Needed*",
  P2: "*FYI*",
};

// ─── Keyboard Builders ────────────────────────────────────────────────────────

function buildItemKeyboard(item: DigestItem): { text: string; callbackData: string }[][] {
  const rows: { text: string; callbackData: string }[][] = [];

  switch (item.source) {
    case "pim":
      if (item.priority === "P0" && item.sourceId) {
        rows.push([
          { text: "Activate Role", callbackData: `digest:pim:activate:${item.sourceId}` },
          { text: "Dismiss", callbackData: `digest:dismiss:${item.id}` },
        ]);
      }
      break;

    case "ado":
      if (item.priority === "P0" && item.sourceId) {
        rows.push([
          { text: "View Build", callbackData: `digest:ado:view:${item.sourceId}` },
          { text: "Dismiss", callbackData: `digest:dismiss:${item.id}` },
        ]);
      }
      break;

    case "email":
      if (item.priority === "P1" && item.sourceId) {
        rows.push([
          { text: "Reply", callbackData: `digest:mail:reply:${item.sourceId}` },
          { text: "Dismiss", callbackData: `digest:dismiss:${item.id}` },
        ]);
      }
      break;

    case "teams":
      if (item.priority === "P1" && item.sourceId) {
        rows.push([
          { text: "Reply", callbackData: `digest:teams:reply:${item.sourceId}` },
          { text: "Dismiss", callbackData: `digest:dismiss:${item.id}` },
        ]);
      }
      break;

    case "obligation":
      if (item.priority === "P1" && item.sourceId) {
        rows.push([
          { text: "Mark Done", callbackData: `digest:ob:done:${item.sourceId}` },
          { text: "Snooze 24h", callbackData: `digest:ob:snooze:${item.sourceId}` },
        ]);
      }
      break;
  }

  return rows;
}

// ─── Format Digest ────────────────────────────────────────────────────────────

export interface FormatResult {
  text: string;
  keyboard: TelegramBot.InlineKeyboardMarkup | null;
}

export function formatDigest(
  items: DigestItem[],
  tier: "thin" | "weekly",
): FormatResult {
  if (items.length === 0) {
    return { text: "", keyboard: null };
  }

  const now = new Date();
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const dayName = days[now.getDay()];
  const monthName = months[now.getMonth()];
  const dateStr = `${dayName} ${monthName} ${now.getDate()}`;
  const tierLabel = tier === "thin" ? "Digest" : "Weekly Digest";
  const timeStr = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;

  const lines: string[] = [`*${tierLabel}* -- ${dateStr} ${timeStr}`, ""];

  // Group by priority
  const byPriority: Record<Priority, DigestItem[]> = { P0: [], P1: [], P2: [] };
  for (const item of items) {
    byPriority[item.priority].push(item);
  }

  // Render each priority section
  for (const priority of ["P0", "P1", "P2"] as Priority[]) {
    const group = byPriority[priority];
    if (group.length === 0) continue;

    lines.push(PRIORITY_HEADERS[priority]);
    for (const item of group) {
      const icon = SOURCE_ICONS[item.source] ?? `[${item.source}]`;
      const detail = item.detail ? `: ${item.detail}` : "";
      lines.push(`  ${icon} ${item.title}${detail}`);
    }
    lines.push("");
  }

  // Footer with source summary
  const activeSources = [...new Set(items.map((i) => i.source))];
  const sourceNames = activeSources
    .map((s) => SOURCE_ICONS[s] ?? s)
    .join(" ");
  lines.push(`_${items.length} items | ${sourceNames}_`);

  let text = lines.join("\n");

  // Build combined keyboard from all actionable items
  const allKeyboardRows: { text: string; callbackData: string }[][] = [];
  for (const item of items) {
    if (item.actionable) {
      const rows = buildItemKeyboard(item);
      allKeyboardRows.push(...rows);
    }
  }

  // Add Dismiss All button if there are any items
  allKeyboardRows.push([
    { text: "Dismiss All", callbackData: "digest:dismiss:all" },
  ]);

  // Truncate text if needed
  if (text.length > TELEGRAM_MAX_LEN) {
    const moreCount = items.length;
    const suffix = `\n... [${moreCount} more items]`;
    text = text.slice(0, TELEGRAM_MAX_LEN - suffix.length) + suffix;
  }

  const keyboard = allKeyboardRows.length > 0
    ? buildKeyboard(allKeyboardRows)
    : null;

  return { text, keyboard };
}

/**
 * Format a standalone P0 urgent notification (not the full digest template).
 */
export function formatP0Alert(item: DigestItem): FormatResult {
  const icon = SOURCE_ICONS[item.source] ?? `[${item.source}]`;
  const detail = item.detail ? `\n${item.detail}` : "";
  const text = `*URGENT* ${icon} ${item.title}${detail}`;

  const keyboardRows = buildItemKeyboard(item);
  const keyboard = keyboardRows.length > 0
    ? buildKeyboard(keyboardRows)
    : null;

  return { text, keyboard };
}

/**
 * Format the Tier 2 weekly LLM synthesis result.
 */
export function formatWeeklySynthesis(synthesis: string): FormatResult {
  const now = new Date();
  const dateStr = now.toISOString().slice(0, 10);
  let text = `*Weekly Digest* -- ${dateStr}\n\n${synthesis}`;

  if (text.length > TELEGRAM_MAX_LEN) {
    text = text.slice(0, TELEGRAM_MAX_LEN - 30) + "\n... [truncated]";
  }

  return { text, keyboard: null };
}
