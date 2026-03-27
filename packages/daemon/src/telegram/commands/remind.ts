import { fleetPost } from "../../fleet-client.js";

const SCHEDULE_SVC_PORT = 4106;

/**
 * Parse a relative time string into an absolute Date.
 * Supports: 30m, 1h, 2h, 3h, 1d, tomorrow
 */
function parseRelativeTime(input: string): Date | null {
  const now = Date.now();
  const trimmed = input.trim().toLowerCase();

  if (trimmed === "tomorrow") {
    const tomorrow = new Date(now);
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(9, 0, 0, 0);
    return tomorrow;
  }

  const match = trimmed.match(/^(\d+)\s*(m|min|mins|minutes?|h|hr|hrs|hours?|d|days?)$/);
  if (!match) return null;

  const amount = parseInt(match[1]!, 10);
  const unit = match[2]![0]!; // m, h, or d

  switch (unit) {
    case "m":
      return new Date(now + amount * 60 * 1000);
    case "h":
      return new Date(now + amount * 60 * 60 * 1000);
    case "d":
      return new Date(now + amount * 24 * 60 * 60 * 1000);
    default:
      return null;
  }
}

/**
 * Parse /remind command arguments.
 * Expected format: /remind [message] [time]
 * The time component is the last word/token.
 * Examples:
 *   /remind check email 1h
 *   /remind standup tomorrow
 *   /remind deploy 30m
 */
function parseRemindArgs(text: string): {
  message: string;
  dueAt: Date;
} | null {
  const trimmed = text.trim();
  if (!trimmed) return null;

  // Try the last token as time
  const lastSpaceIdx = trimmed.lastIndexOf(" ");
  if (lastSpaceIdx === -1) {
    // Single token — try as time with generic message
    const dueAt = parseRelativeTime(trimmed);
    if (dueAt) return { message: "Reminder", dueAt };
    // Not a time — treat as message with 1h default
    return { message: trimmed, dueAt: new Date(Date.now() + 60 * 60 * 1000) };
  }

  const messagePart = trimmed.slice(0, lastSpaceIdx).trim();
  const timePart = trimmed.slice(lastSpaceIdx + 1).trim();

  const dueAt = parseRelativeTime(timePart);
  if (dueAt) {
    return { message: messagePart, dueAt };
  }

  // Last token isn't a time — treat whole string as message, default to 1h
  return {
    message: trimmed,
    dueAt: new Date(Date.now() + 60 * 60 * 1000),
  };
}

interface ReminderResponse {
  id?: string;
  message?: string;
}

/**
 * /remind [message] [time] — set a reminder via schedule-svc
 */
export async function buildRemindReply(argsText?: string): Promise<string> {
  if (!argsText) {
    return [
      "Usage: /remind [message] [time]",
      "",
      "Time formats: 30m, 1h, 2h, 1d, tomorrow",
      "",
      "Examples:",
      "  /remind check email 1h",
      "  /remind standup tomorrow",
      "  /remind deploy 30m",
    ].join("\n");
  }

  const parsed = parseRemindArgs(argsText);
  if (!parsed) {
    return "Could not parse reminder. Usage: /remind [message] [time]";
  }

  const data = (await fleetPost(SCHEDULE_SVC_PORT, "/reminders", {
    message: parsed.message,
    dueAt: parsed.dueAt.toISOString(),
  })) as ReminderResponse;

  const id = data.id ? ` (${data.id.slice(0, 8)})` : "";
  const timeStr = parsed.dueAt.toISOString().slice(0, 16).replace("T", " ");
  return `Reminder set${id}\n  "${parsed.message}"\n  Due: ${timeStr} UTC`;
}
