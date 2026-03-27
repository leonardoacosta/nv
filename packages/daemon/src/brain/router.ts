/**
 * 4-tier message routing cascade.
 * Tier 0: /command (no-op guard — already handled upstream)
 * Tier 1: Regex/keyword matching
 * Tier 2: Embedding similarity (optional — disabled if model fails to load)
 * Tier 3: Full Agent SDK fallback
 */

import type { KeywordRouter } from "./keyword-router.js";
import type { EmbeddingRouter } from "./embedding-router.js";

export type RouteTier = 0 | 1 | 2 | 3;

export interface RouteResult {
  tier: RouteTier;
  tool?: string;
  port?: number;
  params?: Record<string, unknown>;
  confidence: number;
}

export class MessageRouter {
  constructor(
    private readonly keywordRouter: KeywordRouter,
    private readonly embeddingRouter: EmbeddingRouter | null,
  ) {}

  /**
   * Evaluate the cascade in order: Tier 0 -> 1 -> 2 -> 3.
   * Returns on the first match.
   */
  async route(text: string): Promise<RouteResult> {
    // Tier 0: Slash commands — already handled by Telegram adapter's onText handlers.
    // This is a safety guard for messages that start with / but weren't caught.
    if (text.startsWith("/")) {
      return { tier: 0, confidence: 1.0 };
    }

    // Tier 1: Keyword/regex matching
    const keywordMatch = this.keywordRouter.match(text);
    if (keywordMatch) {
      return {
        tier: 1,
        tool: keywordMatch.tool,
        port: keywordMatch.port,
        params: keywordMatch.params,
        confidence: keywordMatch.confidence,
      };
    }

    // Tier 2: Embedding similarity (if available)
    if (this.embeddingRouter) {
      const embeddingMatch = await this.embeddingRouter.match(text);
      if (embeddingMatch) {
        return {
          tier: 2,
          tool: embeddingMatch.tool,
          port: embeddingMatch.port,
          params: {},
          confidence: embeddingMatch.confidence,
        };
      }
    }

    // Tier 3: Fall through to Agent SDK
    return { tier: 3, confidence: 0.0 };
  }
}

/**
 * Format a fleet tool JSON response into a human-readable Telegram message.
 * - If the response has a `text` field, use it directly.
 * - If it is an array, format as a bulleted list.
 * - If it is an object with a `result` field, format the result.
 * - Falls back to JSON code block.
 */
export function formatToolResponse(result: unknown): string {
  if (result === null || result === undefined) {
    return "No data returned.";
  }

  // String result
  if (typeof result === "string") {
    return result;
  }

  // Object with text field
  if (isRecord(result) && typeof result["text"] === "string") {
    return result["text"];
  }

  // Object with result field — unwrap and recurse
  if (isRecord(result) && "result" in result) {
    return formatToolResponse(result["result"]);
  }

  // Object with error field
  if (isRecord(result) && typeof result["error"] === "string") {
    return `Error: ${result["error"]}`;
  }

  // Array of items
  if (Array.isArray(result)) {
    if (result.length === 0) return "No items found.";

    const lines = result.map((item) => {
      if (typeof item === "string") return `- ${item}`;
      if (isRecord(item)) return `- ${formatRecordLine(item)}`;
      return `- ${String(item)}`;
    });

    return lines.join("\n");
  }

  // Object — format key-value pairs
  if (isRecord(result)) {
    const entries = Object.entries(result)
      .filter(([, v]) => v !== null && v !== undefined)
      .map(([k, v]) => `*${k}*: ${typeof v === "object" ? JSON.stringify(v) : String(v)}`);

    if (entries.length > 0) return entries.join("\n");
  }

  // Fallback: JSON code block
  return "```\n" + JSON.stringify(result, null, 2) + "\n```";
}

// ── Helpers ────────────────────────────────────────────────────────────────────

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function formatRecordLine(record: Record<string, unknown>): string {
  // Try common display-friendly fields
  const title =
    record["title"] ?? record["name"] ?? record["subject"] ?? record["summary"];
  const time = record["time"] ?? record["start"] ?? record["date"] ?? record["startTime"];
  const status = record["status"];

  const parts: string[] = [];
  if (title) parts.push(String(title));
  if (time) parts.push(`(${String(time)})`);
  if (status) parts.push(`[${String(status)}]`);

  if (parts.length > 0) return parts.join(" ");

  // Fallback: first 2-3 fields
  const entries = Object.entries(record).slice(0, 3);
  return entries.map(([k, v]) => `${k}: ${String(v)}`).join(", ");
}
