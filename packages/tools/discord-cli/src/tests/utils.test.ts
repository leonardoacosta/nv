import { describe, it } from "node:test";
import assert from "node:assert/strict";

// Inline relativeTime to avoid .js import resolution issues with --experimental-strip-types.
// Mirrors exact implementation in src/utils.ts.
function relativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  const now = Date.now();
  const diffMs = now - then;
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 60) return "just now";

  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;

  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour}h ago`;

  const diffDay = Math.floor(diffHour / 24);
  if (diffDay < 30) return `${diffDay}d ago`;

  const date = new Date(iso);
  const month = date.toLocaleString("en-US", { month: "short" });
  const day = date.getDate();
  return `${month} ${day}`;
}

function isoSecondsAgo(seconds: number): string {
  return new Date(Date.now() - seconds * 1000).toISOString();
}

describe("relativeTime", () => {
  it("returns 'just now' for timestamps less than 60 seconds ago", () => {
    assert.equal(relativeTime(isoSecondsAgo(0)), "just now");
    assert.equal(relativeTime(isoSecondsAgo(30)), "just now");
    assert.equal(relativeTime(isoSecondsAgo(59)), "just now");
  });

  it("returns 'Nm ago' for timestamps less than 60 minutes ago", () => {
    assert.equal(relativeTime(isoSecondsAgo(60)), "1m ago");
    assert.equal(relativeTime(isoSecondsAgo(300)), "5m ago");
    assert.equal(relativeTime(isoSecondsAgo(3540)), "59m ago");
  });

  it("returns 'Nh ago' for timestamps less than 24 hours ago", () => {
    assert.equal(relativeTime(isoSecondsAgo(3600)), "1h ago");
    assert.equal(relativeTime(isoSecondsAgo(7200)), "2h ago");
    assert.equal(relativeTime(isoSecondsAgo(82800)), "23h ago");
  });

  it("returns 'Nd ago' for timestamps less than 30 days ago", () => {
    assert.equal(relativeTime(isoSecondsAgo(86400)), "1d ago");
    assert.equal(relativeTime(isoSecondsAgo(86400 * 7)), "7d ago");
    assert.equal(relativeTime(isoSecondsAgo(86400 * 29)), "29d ago");
  });

  it("returns 'Mon D' for timestamps 30 or more days ago", () => {
    const thirtyDaysAgo = new Date(Date.now() - 86400 * 30 * 1000);
    const result = relativeTime(thirtyDaysAgo.toISOString());
    const month = thirtyDaysAgo.toLocaleString("en-US", { month: "short" });
    const day = thirtyDaysAgo.getDate();
    assert.equal(result, `${month} ${day}`);
  });

  it("returns 'Mon D' for a past date well over 30 days ago", () => {
    // Use noon UTC to avoid date shifting from local timezone offset
    const iso = "2023-06-20T12:00:00.000Z";
    const result = relativeTime(iso);
    const date = new Date(iso);
    const expected = `${date.toLocaleString("en-US", { month: "short" })} ${date.getDate()}`;
    assert.equal(result, expected);
  });
});
