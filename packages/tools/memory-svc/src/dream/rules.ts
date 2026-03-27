import { existsSync } from "node:fs";
import { homedir } from "node:os";
import type { RuleResult, RuleStats } from "./types.js";

// ---------------------------------------------------------------------------
// Levenshtein distance (simple dp -- only used on lines already pre-filtered
// to length >= 20 chars, so no pathological perf case).
// ---------------------------------------------------------------------------
function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () =>
    new Array<number>(n + 1).fill(0),
  );
  for (let i = 0; i <= m; i++) dp[i]![0] = i;
  for (let j = 0; j <= n; j++) dp[0]![j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      dp[i]![j] = Math.min(
        dp[i - 1]![j]! + 1,
        dp[i]![j - 1]! + 1,
        dp[i - 1]![j - 1]! + cost,
      );
    }
  }
  return dp[m]![n]!;
}

// ---------------------------------------------------------------------------
// Rule 1 -- Deduplication
// ---------------------------------------------------------------------------
function dedup(lines: string[]): { lines: string[]; count: number } {
  let count = 0;
  const kept: string[] = [];
  const seen = new Set<string>();

  for (const line of lines) {
    const trimmed = line.trimEnd();

    // Exact duplicate check (case-sensitive on trimmed line)
    if (seen.has(trimmed)) {
      count++;
      continue;
    }

    // Near-duplicate check (only for lines >= 20 chars)
    if (trimmed.length >= 20) {
      let isDuplicate = false;
      for (let i = kept.length - 1; i >= 0; i--) {
        const existing = kept[i]!.trimEnd();
        if (existing.length < 20) continue;
        const maxLen = Math.max(trimmed.length, existing.length);
        const threshold = Math.floor(maxLen * 0.1);
        const dist = levenshtein(trimmed, existing);
        if (dist < threshold) {
          // Keep the longer variant; replace if new line is longer
          if (trimmed.length > existing.length) {
            seen.delete(existing);
            seen.add(trimmed);
            kept[i] = line;
          }
          isDuplicate = true;
          count++;
          break;
        }
      }
      if (isDuplicate) continue;
    }

    seen.add(trimmed);
    kept.push(line);
  }

  return { lines: kept, count };
}

// ---------------------------------------------------------------------------
// Rule 2 -- Date normalization
// ---------------------------------------------------------------------------

/** Map of relative-date patterns to a function computing the replacement string. */
const RELATIVE_DATE_PATTERNS: {
  pattern: RegExp;
  resolve: (ref: Date) => string;
}[] = [
  {
    pattern: /\bthis morning\b/gi,
    resolve: (ref) => formatDate(ref),
  },
  {
    pattern: /\btoday\b/gi,
    resolve: (ref) => formatDate(ref),
  },
  {
    pattern: /\byesterday\b/gi,
    resolve: (ref) => {
      const d = new Date(ref);
      d.setDate(d.getDate() - 1);
      return formatDate(d);
    },
  },
  {
    pattern: /\ba few days ago\b/gi,
    resolve: (ref) => {
      const d = new Date(ref);
      d.setDate(d.getDate() - 3);
      return `around ${formatDate(d)}`;
    },
  },
  {
    pattern: /\blast week\b/gi,
    resolve: (ref) => {
      const d = new Date(ref);
      d.setDate(d.getDate() - 7);
      return `the week of ${formatDate(d)}`;
    },
  },
  {
    pattern: /\blast month\b/gi,
    resolve: (ref) => {
      const d = new Date(ref);
      d.setMonth(d.getMonth() - 1);
      return formatDate(d, { monthOnly: true });
    },
  },
  {
    pattern: /\brecently\b/gi,
    resolve: (ref) => {
      const d = new Date(ref);
      d.setDate(d.getDate() - 3);
      return `around ${formatDate(d)}`;
    },
  },
];

function formatDate(d: Date, opts?: { monthOnly?: boolean }): string {
  if (opts?.monthOnly) {
    return d.toLocaleDateString("en-US", { year: "numeric", month: "long" });
  }
  return d.toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function normalizeDates(
  content: string,
  updatedAt: Date,
): { content: string; count: number } {
  let count = 0;
  let result = content;
  for (const { pattern, resolve } of RELATIVE_DATE_PATTERNS) {
    const replacement = resolve(updatedAt);
    result = result.replace(pattern, () => {
      count++;
      return replacement;
    });
  }
  return { content: result, count };
}

// ---------------------------------------------------------------------------
// Rule 3 -- Whitespace cleanup
// ---------------------------------------------------------------------------
function cleanWhitespace(content: string): { content: string; changed: number } {
  let changed = 0;
  const original = content;

  // Trim trailing whitespace per line
  let result = content.replace(/[ \t]+$/gm, (match) => {
    if (match.length > 0) changed++;
    return "";
  });

  // Collapse 3+ consecutive blank lines to 2
  result = result.replace(/(\n\s*){3,}\n/g, (match) => {
    changed++;
    return "\n\n\n"; // 2 blank lines = 3 newlines
  });

  // Remove leading blank lines
  const beforeLeading = result;
  result = result.replace(/^\s*\n/, "");
  if (result !== beforeLeading) changed++;

  // Remove trailing blank lines
  const beforeTrailing = result;
  result = result.replace(/\n\s*$/, "\n");
  if (result !== beforeTrailing) changed++;

  // If nothing changed, reset count to avoid false positives
  if (result === original) changed = 0;

  return { content: result, changed };
}

// ---------------------------------------------------------------------------
// Rule 4 -- Stale path removal
// ---------------------------------------------------------------------------

const PATH_LINE_PATTERNS = [
  // Line starts with a path
  /^(~\/\S+)/,
  /^(\/home\/\S+)/,
  /^(packages\/\S+)/,
  /^(apps\/\S+)/,
  // Bullet list item whose content is primarily a path
  /^[-*]\s+(~\/\S+)\s*$/,
  /^[-*]\s+(\/home\/\S+)\s*$/,
  /^[-*]\s+(packages\/\S+)\s*$/,
  /^[-*]\s+(apps\/\S+)\s*$/,
];

function expandPath(p: string): string {
  if (p.startsWith("~/")) {
    return homedir() + p.slice(1);
  }
  return p;
}

function removeStalePaths(lines: string[]): { lines: string[]; count: number } {
  let count = 0;
  const kept: string[] = [];

  for (const line of lines) {
    let removed = false;
    for (const re of PATH_LINE_PATTERNS) {
      const m = line.match(re);
      if (m?.[1]) {
        const resolved = expandPath(m[1]);
        if (!existsSync(resolved)) {
          count++;
          removed = true;
          break;
        }
      }
    }
    if (!removed) kept.push(line);
  }

  return { lines: kept, count };
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Apply all deterministic consolidation rules to a topic's content.
 *
 * Order: dedup -> date normalization -> whitespace cleanup -> stale path removal -> budget check.
 */
export function applyRules(
  content: string,
  updatedAt: Date,
  topicMaxKb: number,
): RuleResult {
  // 1. Deduplication
  const lines = content.split("\n");
  const dedupResult = dedup(lines);

  // 2. Date normalization
  const dateResult = normalizeDates(dedupResult.lines.join("\n"), updatedAt);

  // 3. Whitespace cleanup
  const wsResult = cleanWhitespace(dateResult.content);

  // 4. Stale path removal
  const pathResult = removeStalePaths(wsResult.content.split("\n"));
  const finalContent = pathResult.lines.join("\n");

  // 5. Budget check
  const sizeBytes = Buffer.byteLength(finalContent, "utf-8");
  const needsLlm = sizeBytes > topicMaxKb * 1024;

  const stats: RuleStats = {
    dedupedLines: dedupResult.count,
    datesNormalized: dateResult.count,
    stalePathsRemoved: pathResult.count,
    whitespaceFixed: wsResult.changed,
  };

  return { content: finalContent, needsLlm, stats };
}
