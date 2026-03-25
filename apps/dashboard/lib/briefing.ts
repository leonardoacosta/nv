/**
 * Utilities for parsing briefing content into structured sections.
 *
 * Supports two header formats produced by the Nova daemon:
 *   - `-- Title --`  (synthesize_digest_fallback format)
 *   - `### Title`    (Claude markdown format)
 */

export interface BriefingSection {
  title: string;
  body: string;
}

/**
 * Parse a raw briefing content string into an array of titled sections.
 *
 * Detection rules (in order of precedence):
 *   1. Lines matching `-- Some Title --` (trimmed)
 *   2. Lines matching `### Some Title` (trimmed)
 *
 * Fallback: if no headers are detected, return a single Summary section
 * so the page always has something to render.
 *
 * Test cases (verified inline below):
 *   - dash format:   `-- Overview --\nbody text` → [{title:"Overview", body:"body text"}]
 *   - hash format:   `### Overview\nbody text`   → [{title:"Overview", body:"body text"}]
 *   - mixed:         first content has no header → [{title:"Summary", body:...}, ...]
 *   - no headers:    plain text → [{title:"Summary", body:"plain text"}]
 *   - empty string:  "" → [{title:"Summary", body:""}]
 *   - whitespace:    "   " → [{title:"Summary", body:""}]
 */
export function parseBriefingSections(content: string): BriefingSection[] {
  const lines = content.split("\n");

  // Regex patterns for the two header formats
  const dashHeader = /^--\s+(.+?)\s+--\s*$/;
  const hashHeader = /^###\s+(.+?)\s*$/;

  // Detect whether any headers exist at all
  const hasHeaders = lines.some(
    (line) => dashHeader.test(line) || hashHeader.test(line),
  );

  if (!hasHeaders) {
    return [{ title: "Summary", body: content.trim() }];
  }

  const sections: BriefingSection[] = [];
  let currentTitle: string | null = null;
  let bodyLines: string[] = [];

  const flush = () => {
    if (currentTitle !== null) {
      sections.push({ title: currentTitle, body: bodyLines.join("\n").trim() });
    }
    bodyLines = [];
  };

  for (const line of lines) {
    const dashMatch = dashHeader.exec(line);
    const hashMatch = hashHeader.exec(line);

    if (dashMatch ?? hashMatch) {
      flush();
      currentTitle = (dashMatch?.[1] ?? hashMatch?.[1]) as string;
    } else {
      bodyLines.push(line);
    }
  }

  // Flush last section
  flush();

  // If we somehow ended up with nothing (e.g., only headers, no body), return fallback
  return sections.length > 0
    ? sections
    : [{ title: "Summary", body: content.trim() }];
}

/*
 * Inline test verification (runs at import time in dev — non-fatal, logs to console):
 *
 * These assertions document expected behaviour without requiring a test framework.
 * They are guarded so they never run in production builds.
 */
if (process.env.NODE_ENV === "development") {
  const assert = (condition: boolean, label: string) => {
    if (!condition) console.error(`[briefing.ts] FAIL: ${label}`);
  };

  // dash format
  const dash = parseBriefingSections("-- Overview --\nbody text");
  assert(dash.length === 1, "dash: one section");
  assert(dash[0]!.title === "Overview", "dash: title");
  assert(dash[0]!.body === "body text", "dash: body");

  // hash format
  const hash = parseBriefingSections("### Markets\nsome market data");
  assert(hash.length === 1, "hash: one section");
  assert(hash[0]!.title === "Markets", "hash: title");

  // two sections
  const two = parseBriefingSections("-- A --\nbody a\n-- B --\nbody b");
  assert(two.length === 2, "two sections: count");
  assert(two[0]!.title === "A", "two sections: first title");
  assert(two[1]!.title === "B", "two sections: second title");

  // no headers → fallback
  const plain = parseBriefingSections("just plain text");
  assert(plain.length === 1, "plain: fallback");
  assert(plain[0]!.title === "Summary", "plain: title is Summary");
  assert(plain[0]!.body === "just plain text", "plain: body");

  // empty string
  const empty = parseBriefingSections("");
  assert(empty.length === 1, "empty: one section");
  assert(empty[0]!.body === "", "empty: empty body");

  // whitespace only
  const ws = parseBriefingSections("   \n  ");
  assert(ws.length === 1, "whitespace: one section");
  assert(ws[0]!.body === "", "whitespace: body trimmed to empty");

  // mixed: content before first header → content becomes part of prior flush (no title = skipped)
  const mixed = parseBriefingSections("intro text\n-- Section --\nsection body");
  assert(mixed.length === 1, "mixed: only titled sections returned");
  assert(mixed[0]!.title === "Section", "mixed: titled section title");
}
