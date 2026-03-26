/**
 * Converts an ISO 8601 timestamp to a human-readable relative time string.
 *
 * Buckets:
 *   < 60s     → "just now"
 *   < 60m     → "Nm ago"
 *   < 24h     → "Nh ago"
 *   < 30d     → "Nd ago"
 *   older     → "Mon D" (e.g. "Mar 15")
 */
export function relativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  const now = Date.now();
  const diffMs = now - then;
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 60) {
    return "just now";
  }

  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) {
    return `${diffMin}m ago`;
  }

  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) {
    return `${diffHour}h ago`;
  }

  const diffDay = Math.floor(diffHour / 24);
  if (diffDay < 30) {
    return `${diffDay}d ago`;
  }

  const date = new Date(iso);
  const month = date.toLocaleString("en-US", { month: "short" });
  const day = date.getDate();
  return `${month} ${day}`;
}
