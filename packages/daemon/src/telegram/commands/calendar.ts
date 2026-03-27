import { fleetGet } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const GRAPH_SVC_PORT = 4107;

interface CalendarEvent {
  subject?: string;
  start?: string | { dateTime?: string };
  end?: string | { dateTime?: string };
  location?: string | { displayName?: string };
  isAllDay?: boolean;
}

function formatTime(raw?: string | { dateTime?: string }): string {
  if (!raw) return "?";
  const str = typeof raw === "string" ? raw : raw.dateTime ?? "";
  if (!str) return "?";
  try {
    const d = new Date(str);
    return d.toTimeString().slice(0, 5);
  } catch {
    return str.slice(0, 5);
  }
}

function formatLocation(
  loc?: string | { displayName?: string },
): string | null {
  if (!loc) return null;
  if (typeof loc === "string") return loc;
  return loc.displayName ?? null;
}

/**
 * /calendar — today's calendar events from graph-svc
 */
export async function buildCalendarReply(): Promise<string> {
  const data = await fleetGet(GRAPH_SVC_PORT, "/calendar/today");
  const events = (
    Array.isArray(data)
      ? data
      : Array.isArray((data as { events?: unknown }).events)
        ? (data as { events: unknown[] }).events
        : Array.isArray((data as { value?: unknown }).value)
          ? (data as { value: unknown[] }).value
          : []
  ) as CalendarEvent[];

  if (events.length === 0) {
    return "No calendar events for today.";
  }

  const header = `Calendar - Today (${events.length} events)\n${"─".repeat(32)}\n`;
  const lines = events.map((e) => {
    const subject = e.subject ?? "(no subject)";
    if (e.isAllDay) {
      const loc = formatLocation(e.location);
      return `  [All day] ${subject}${loc ? ` @ ${loc}` : ""}`;
    }
    const start = formatTime(e.start);
    const end = formatTime(e.end);
    const loc = formatLocation(e.location);
    return `  ${start}-${end} ${subject}${loc ? ` @ ${loc}` : ""}`;
  });

  return truncate(header + lines.join("\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
