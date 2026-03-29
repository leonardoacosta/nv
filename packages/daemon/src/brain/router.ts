/**
 * 4-tier message routing cascade.
 * Tier 0: /command (no-op guard — already handled upstream)
 * Tier 1: Regex/keyword matching
 * Tier 2: Embedding similarity (optional — disabled if model fails to load)
 * Tier 3: Full Agent SDK fallback
 */

import type { KeywordRouter } from "./keyword-router.js";
import type { EmbeddingRouter } from "./embedding-router.js";
import { detectSignals } from "../features/obligations/signal-detector.js";
import { detectObligationLightweight } from "../features/obligations/detector.js";
import type { ObligationStore } from "../features/obligations/store.js";
import { ObligationStatus } from "../features/obligations/types.js";
import { logger } from "../logger.js";

export type RouteTier = 0 | 1 | 2 | 3;

export interface RouteResult {
  tier: RouteTier;
  tool?: string;
  port?: number;
  params?: Record<string, unknown>;
  confidence: number;
}

// ─── In-memory hourly rate limiter ────────────────────────────────────────────

const MAX_DETECTION_JOBS_PER_HOUR = 10;

class HourlyRateLimiter {
  private count = 0;
  private windowStart = Date.now();

  private resetIfNeeded(): void {
    const now = Date.now();
    if (now - this.windowStart >= 3_600_000) {
      this.count = 0;
      this.windowStart = now;
    }
  }

  tryConsume(): boolean {
    this.resetIfNeeded();
    if (this.count >= MAX_DETECTION_JOBS_PER_HOUR) {
      return false;
    }
    this.count++;
    return true;
  }

  get remaining(): number {
    this.resetIfNeeded();
    return Math.max(0, MAX_DETECTION_JOBS_PER_HOUR - this.count);
  }
}

const detectionRateLimiter = new HourlyRateLimiter();

// ─── Post-routing obligation hook ─────────────────────────────────────────────

export interface ObligationHookOptions {
  userMessage: string;
  toolResponse: string;
  channel: string;
  routeResult: RouteResult;
  store: ObligationStore;
  gatewayKey?: string;
}

/**
 * Fire-and-forget post-routing obligation detection hook.
 * Runs after Tier 1/2 dispatch. If signals are detected and the rate limit
 * allows, enqueues a lightweight Haiku obligation detection job.
 * Never throws — all errors are logged.
 */
export function runPostRoutingObligationHook(
  options: ObligationHookOptions,
): void {
  const { userMessage, toolResponse, channel, routeResult, store, gatewayKey } = options;

  if (routeResult.tier !== 1 && routeResult.tier !== 2) {
    return;
  }

  // Fire-and-forget: do not await
  void (async () => {
    try {
      const signalResult = detectSignals(userMessage);

      if (!signalResult.detected) {
        return;
      }

      if (!detectionRateLimiter.tryConsume()) {
        logger.debug(
          { remaining: 0 },
          "Obligation detection rate limit reached — skipping",
        );
        return;
      }

      logger.debug(
        { signals: signalResult.signals, confidence: signalResult.confidence, tier: routeResult.tier },
        "Obligation signals detected — running lightweight Haiku detection",
      );

      const detectionSource = routeResult.tier === 1 ? "tier1" as const : "tier2" as const;

      const result = await detectObligationLightweight({
        userMessage,
        toolResponse,
        channel,
        detectionSource,
        routedTool: routeResult.tool,
        signalResult,
        gatewayKey,
      });

      if (!result) {
        return;
      }

      await store.create({
        detectedAction: result.detectedAction,
        owner: result.owner,
        status: ObligationStatus.Open,
        priority: result.priority,
        projectCode: result.projectCode,
        sourceChannel: channel,
        sourceMessage: userMessage,
        deadline: result.deadline,
        detectionSource: result.detectionSource,
        routedTool: result.routedTool,
      });

      logger.info(
        {
          action: result.detectedAction,
          owner: result.owner,
          tier: routeResult.tier,
          tool: routeResult.tool,
          rateLimitRemaining: detectionRateLimiter.remaining,
        },
        "Lightweight obligation detected and created",
      );
    } catch (err: unknown) {
      logger.warn(
        { err: err instanceof Error ? err.message : String(err) },
        "Post-routing obligation hook failed",
      );
    }
  })();
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
