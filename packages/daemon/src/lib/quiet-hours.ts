/**
 * Determines whether a given moment falls within a quiet-hours window.
 *
 * @param now        - The timestamp to evaluate (use `new Date()` for the current time)
 * @param quietStart - Quiet period start in 24-hour `HH:MM` format (e.g. `"22:00"`)
 * @param quietEnd   - Quiet period end in 24-hour `HH:MM` format (e.g. `"07:00"`)
 * @returns `true` if `now` falls inside the quiet window, `false` otherwise
 *
 * Handles three cases:
 * - **Same-day window** (start < end): e.g. `08:00–17:00` — active when within the range
 * - **Midnight-wrap window** (start > end): e.g. `22:00–07:00` — active when at or after
 *   start OR before end
 * - **Zero-length window** (start === end): always returns `false` (quiet hours disabled)
 *
 * Uses local system time (no UTC conversion). Ensure the daemon's process timezone
 * matches the user's timezone for correct behaviour.
 */
export function isQuietHours(
  now: Date,
  quietStart: string,
  quietEnd: string,
): boolean {
  const [startHour = 0, startMin = 0] = quietStart.split(":").map(Number);
  const [endHour = 0, endMin = 0] = quietEnd.split(":").map(Number);

  const startMinutes = startHour * 60 + startMin;
  const endMinutes = endHour * 60 + endMin;
  const nowMinutes = now.getHours() * 60 + now.getMinutes();

  // Zero-length window: quiet hours are disabled
  if (startMinutes === endMinutes) return false;

  if (startMinutes < endMinutes) {
    // Normal same-day window: e.g. 08:00–17:00
    return nowMinutes >= startMinutes && nowMinutes < endMinutes;
  }

  // Midnight-wrap window: e.g. 22:00–07:00
  // Active when: nowMinutes >= 22:00 OR nowMinutes < 07:00
  return nowMinutes >= startMinutes || nowMinutes < endMinutes;
}
