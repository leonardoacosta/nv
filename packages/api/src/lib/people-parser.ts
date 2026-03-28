/**
 * Parser for the memory `people` topic.
 *
 * The people topic is a freeform text blob written by Nova. This parser uses
 * heuristic matching to extract structured PersonProfile records from it.
 * It is designed to degrade gracefully: unrecognized content is captured as
 * raw notes rather than failing.
 *
 * Moved from apps/dashboard/lib/entity-resolution/people-parser.ts so that
 * the API package can use it for server-side materialization and resolution.
 */

export interface PersonProfile {
  name: string;
  channelIds: Record<string, string>;
  role: string | null;
  notes: string;
}

// ── Pattern constants ─────────────────────────────────────────────────────

/**
 * Telegram user IDs: numeric strings, 6–15 digits.
 * Discord snowflakes: numeric strings, 17–20 digits.
 * We distinguish them by length: ≤16 = Telegram, 17–20 = Discord.
 */
const TELEGRAM_ID_RE = /\b(\d{6,16})\b/g;
const DISCORD_ID_RE = /\b(\d{17,20})\b/g;

/** Teams IDs always contain an `@` sign (email-style). */
const TEAMS_ID_RE = /\b([\w.+-]+@[\w.-]+\.\w+)\b/g;

/** Role keywords to detect from profile text. */
const ROLE_KEYWORDS = [
  "PM",
  "engineer",
  "manager",
  "lead",
  "developer",
  "designer",
  "architect",
  "analyst",
  "director",
  "consultant",
  "coordinator",
  "specialist",
  "researcher",
  "founder",
  "CEO",
  "CTO",
  "VP",
  "head of",
  "intern",
  "contractor",
];

// ── Section header detection ──────────────────────────────────────────────

/** Returns true if the line looks like a person name header. */
function isNameHeader(line: string): boolean {
  const trimmed = line.trim();
  if (!trimmed) return false;

  // ## Name Header
  if (/^#{1,3}\s+\S/.test(trimmed)) return true;

  // **Name** (bold markdown)
  if (/^\*\*[^*]+\*\*\s*$/.test(trimmed)) return true;

  // ALL CAPS NAMES (2+ words, no digits, short enough to be a name)
  if (/^[A-Z][A-Z\s]{2,40}$/.test(trimmed) && !/\d/.test(trimmed)) return true;

  // --- Name --- (HR-style header)
  if (/^---\s*.+\s*---$/.test(trimmed)) return true;

  return false;
}

/** Extract the clean name string from a header line. */
function extractNameFromHeader(line: string): string {
  const trimmed = line.trim();

  // ## Name Header
  const hashMatch = trimmed.match(/^#{1,3}\s+(.+)$/);
  if (hashMatch) return hashMatch[1].trim();

  // **Name**
  const boldMatch = trimmed.match(/^\*\*([^*]+)\*\*/);
  if (boldMatch) return boldMatch[1].trim();

  // --- Name ---
  const hrMatch = trimmed.match(/^---\s*(.+?)\s*---$/);
  if (hrMatch) return hrMatch[1].trim();

  // ALL CAPS — title-case it for readability
  if (/^[A-Z\s]{3,}$/.test(trimmed)) {
    return trimmed
      .toLowerCase()
      .replace(/\b\w/g, (c) => c.toUpperCase());
  }

  return trimmed;
}

// ── Channel ID extraction ─────────────────────────────────────────────────

function extractChannelIds(text: string): Record<string, string> {
  const ids: Record<string, string> = {};

  // Teams IDs (email-style) — check first so emails aren't misclassified
  for (const match of text.matchAll(TEAMS_ID_RE)) {
    ids["teams"] = match[1];
  }

  // Discord snowflakes (17–20 digits) — before Telegram to avoid overlap
  for (const match of text.matchAll(DISCORD_ID_RE)) {
    ids["discord"] = match[1];
  }

  // Telegram IDs (6–16 digits) — anything not already captured
  for (const match of text.matchAll(TELEGRAM_ID_RE)) {
    if (!Object.values(ids).includes(match[1])) {
      ids["telegram"] = match[1];
    }
  }

  return ids;
}

// ── Role extraction ───────────────────────────────────────────────────────

function extractRole(text: string): string | null {
  const lower = text.toLowerCase();
  for (const keyword of ROLE_KEYWORDS) {
    if (lower.includes(keyword.toLowerCase())) {
      return keyword;
    }
  }
  return null;
}

// ── Main parser ───────────────────────────────────────────────────────────

/**
 * Parse the `people` memory topic text blob into structured PersonProfile[].
 *
 * Strategy:
 * 1. Split the content into sections, each beginning with a name header line.
 * 2. For each section, extract channel IDs, role, and notes.
 * 3. Sections with no identifiable name are skipped.
 */
export function parsePeopleMemory(content: string): PersonProfile[] {
  if (!content || !content.trim()) return [];

  const lines = content.split("\n");
  const profiles: PersonProfile[] = [];

  // Accumulate lines into sections delimited by name headers
  type Section = { headerLine: string; bodyLines: string[] };
  const sections: Section[] = [];
  let current: Section | null = null;

  for (const line of lines) {
    if (isNameHeader(line)) {
      if (current) sections.push(current);
      current = { headerLine: line, bodyLines: [] };
    } else if (current) {
      current.bodyLines.push(line);
    }
    // Lines before the first header are discarded (typically a topic intro)
  }
  if (current) sections.push(current);

  for (const section of sections) {
    const name = extractNameFromHeader(section.headerLine);
    if (!name) continue;

    const bodyText = section.bodyLines.join("\n");
    const channelIds = extractChannelIds(bodyText);
    const role = extractRole(bodyText);

    // Notes = full body text, trimmed
    const notes = bodyText.trim();

    profiles.push({ name, channelIds, role, notes });
  }

  return profiles;
}
