import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";
import { socksGet, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";

const GRAPH_BASE = "https://graph.microsoft.com/v1.0";
const OUTLOOK_SCRIPT = "graph-outlook.ps1";

// ── Formatters ─────────────────────────────────────────────────────────

interface GraphEvent {
  subject?: string;
  start?: { dateTime?: string; timeZone?: string };
  end?: { dateTime?: string; timeZone?: string };
  location?: { displayName?: string };
  organizer?: { emailAddress?: { name?: string; address?: string } };
  attendees?: Array<{
    emailAddress?: { name?: string; address?: string };
    status?: { response?: string };
  }>;
  isAllDay?: boolean;
  isCancelled?: boolean;
  showAs?: string;
  webLink?: string;
}

function formatEvent(ev: GraphEvent): string {
  const start = ev.start?.dateTime
    ? new Date(ev.start.dateTime).toLocaleString("en-US", {
        weekday: "short",
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : "Unknown time";
  const end = ev.end?.dateTime
    ? new Date(ev.end.dateTime).toLocaleString("en-US", {
        hour: "numeric",
        minute: "2-digit",
      })
    : "";
  const time = ev.isAllDay ? "All Day" : `${start} - ${end}`;
  const location = ev.location?.displayName ? ` | ${ev.location.displayName}` : "";
  const organizer = ev.organizer?.emailAddress?.name
    ? ` | Organizer: ${ev.organizer.emailAddress.name}`
    : "";
  const status = ev.isCancelled ? " [CANCELLED]" : ev.showAs === "tentative" ? " [Tentative]" : "";
  return `${ev.subject ?? "(No subject)"}${status}\n  ${time}${location}${organizer}`;
}

function formatEvents(events: GraphEvent[]): string {
  if (events.length === 0) return "No events found.";
  return events.map((ev, i) => `${i + 1}. ${formatEvent(ev)}`).join("\n\n");
}

// ── Helpers ────────────────────────────────────────────────────────────

function todayRange(): { start: string; end: string } {
  const now = new Date();
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const end = new Date(start);
  end.setDate(end.getDate() + 1);
  return { start: start.toISOString(), end: end.toISOString() };
}

function daysRange(days: number): { start: string; end: string } {
  const now = new Date();
  const end = new Date(now);
  end.setDate(end.getDate() + days);
  return { start: now.toISOString(), end: end.toISOString() };
}

async function fetchCalendarView(
  config: ServiceConfig,
  start: string,
  end: string,
  top?: number,
  orderby?: string,
): Promise<GraphEvent[]> {
  const params = new URLSearchParams({
    startDateTime: start,
    endDateTime: end,
    $select: "subject,start,end,location,organizer,attendees,isAllDay,isCancelled,showAs,webLink",
    $orderby: orderby ?? "start/dateTime",
  });
  if (top) params.set("$top", String(top));

  const url = `${GRAPH_BASE}/me/calendarView?${params}`;
  const token = await getO365Token(config.cloudpcHost);

  try {
    const raw = await socksGet(url, token);
    const data = JSON.parse(raw) as { value?: GraphEvent[] };
    return data.value ?? [];
  } catch (err) {
    // On 401, clear token and retry once
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const freshToken = await getO365Token(config.cloudpcHost);
      const raw = await socksGet(url, freshToken);
      const data = JSON.parse(raw) as { value?: GraphEvent[] };
      return data.value ?? [];
    }
    throw err;
  }
}

// ── Tool implementations ───────────────────────────────────────────────

/**
 * Get today's calendar events from Outlook.
 */
export async function calendarToday(config: ServiceConfig): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(
      config.cloudpcHost,
      config.cloudpcUserPath,
      OUTLOOK_SCRIPT,
      "-Action CalendarToday",
    );
  }

  const { start, end } = todayRange();
  const events = await fetchCalendarView(config, start, end);
  return formatEvents(events);
}

/**
 * Get upcoming calendar events for the specified number of days.
 * @param days Number of days to look ahead (1-14, default 7)
 */
export async function calendarUpcoming(
  config: ServiceConfig,
  days: number = 7,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(
      config.cloudpcHost,
      config.cloudpcUserPath,
      OUTLOOK_SCRIPT,
      `-Action CalendarUpcoming -Days ${days}`,
    );
  }

  const { start, end } = daysRange(days);
  const events = await fetchCalendarView(config, start, end);
  return formatEvents(events);
}

/**
 * Get the next upcoming calendar event.
 */
export async function calendarNext(config: ServiceConfig): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(
      config.cloudpcHost,
      config.cloudpcUserPath,
      OUTLOOK_SCRIPT,
      "-Action CalendarNext",
    );
  }

  const { start, end } = daysRange(7);
  const events = await fetchCalendarView(config, start, end, 1);
  if (events.length === 0) return "No upcoming events.";
  return formatEvent(events[0]!);
}
