import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";

const OUTLOOK_SCRIPT = "graph-outlook.ps1";

/**
 * Get today's calendar events from Outlook.
 */
export async function calendarToday(config: ServiceConfig): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    "-Action CalendarToday",
  );
}

/**
 * Get upcoming calendar events for the specified number of days.
 * @param days Number of days to look ahead (1-14, default 7)
 */
export async function calendarUpcoming(
  config: ServiceConfig,
  days: number = 7,
): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    `-Action CalendarUpcoming -Days ${days}`,
  );
}

/**
 * Get the next upcoming calendar event.
 */
export async function calendarNext(config: ServiceConfig): Promise<string> {
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    OUTLOOK_SCRIPT,
    "-Action CalendarNext",
  );
}
