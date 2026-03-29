import Anthropic from "@anthropic-ai/sdk";
import type { Logger } from "pino";
import type { SignalResult } from "./signal-detector.js";
import type { DetectionSource } from "./types.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface DetectedObligation {
  detectedAction: string;
  owner: "nova" | "leo";
  priority: 1 | 2 | 3;
  projectCode: string | null;
  deadline: Date | null;
}

// ─── Raw JSON shape from Claude ───────────────────────────────────────────────

interface RawObligation {
  detectedAction?: unknown;
  owner?: unknown;
  priority?: unknown;
  projectCode?: unknown;
  deadline?: unknown;
}

// ─── Anthropic client factory ─────────────────────────────────────────────────

// TODO(extract-agent-query-factory): Replace with createAgentQuery() once that spec lands.
// This file uses the raw Anthropic SDK — different auth flow from the rest of the daemon.
function createClient(gatewayKey: string): Anthropic {
  return new Anthropic({
    baseURL: "https://ai-gateway.vercel.sh",
    defaultHeaders: {
      "x-ai-gateway-api-key": `Bearer ${gatewayKey}`,
    },
  });
}

// ─── Prompt ───────────────────────────────────────────────────────────────────

function buildDetectionPrompt(
  userMessage: string,
  novaResponse: string,
  channel: string,
): string {
  return `You are analyzing a conversation between a user and Nova (an AI assistant) to identify obligations.

Channel: ${channel}

User message:
${userMessage}

Nova's response:
${novaResponse}

Task: Did Nova make any commitments, agree to do anything, or defer any action? If yes, list each as a structured obligation.

Return a JSON array of obligations. Each obligation must have:
- "detectedAction": imperative verb phrase describing what must be done (e.g. "Review the Jira backlog")
- "owner": "nova" if Nova committed to do it, "leo" if the user was asked to do something
- "priority": 1 (urgent), 2 (normal), or 3 (low)
- "projectCode": project identifier string if discernible from context, or null
- "deadline": ISO 8601 date-time string if a deadline was mentioned, or null

If there are no obligations, return an empty array: []

Respond with ONLY the JSON array, no other text.`;
}

// ─── JSON extraction helper ───────────────────────────────────────────────────

function extractJsonArray(text: string): string {
  // Strip markdown code fences if present
  const fenceMatch = /```(?:json)?\s*([\s\S]*?)```/.exec(text);
  if (fenceMatch?.[1]) {
    return fenceMatch[1].trim();
  }

  // Find the first '[' and last ']'
  const start = text.indexOf("[");
  const end = text.lastIndexOf("]");
  if (start !== -1 && end !== -1 && end > start) {
    return text.slice(start, end + 1);
  }

  return text.trim();
}

// ─── Validation ───────────────────────────────────────────────────────────────

function isValidOwner(value: unknown): value is "nova" | "leo" {
  return value === "nova" || value === "leo";
}

function isValidPriority(value: unknown): value is 1 | 2 | 3 {
  return value === 1 || value === 2 || value === 3;
}

function parseRawObligation(raw: RawObligation): DetectedObligation | null {
  if (typeof raw.detectedAction !== "string" || !raw.detectedAction) {
    return null;
  }
  if (!isValidOwner(raw.owner)) {
    return null;
  }
  if (!isValidPriority(raw.priority)) {
    return null;
  }

  const projectCode =
    typeof raw.projectCode === "string" ? raw.projectCode : null;

  let deadline: Date | null = null;
  if (typeof raw.deadline === "string" && raw.deadline) {
    const parsed = new Date(raw.deadline);
    deadline = isNaN(parsed.getTime()) ? null : parsed;
  }

  return {
    detectedAction: raw.detectedAction,
    owner: raw.owner,
    priority: raw.priority,
    projectCode,
    deadline,
  };
}

// ─── Lightweight Haiku detection ─────────────────────────────────────────────

export interface LightweightDetectionInput {
  userMessage: string;
  toolResponse: string;
  channel: string;
  detectionSource: DetectionSource;
  routedTool?: string;
  signalResult: SignalResult;
  gatewayKey?: string;
}

export interface LightweightDetectionResult {
  detectedAction: string;
  owner: "nova" | "leo";
  priority: 1 | 2 | 3;
  projectCode: string | null;
  deadline: Date | null;
  detectionSource: DetectionSource;
  routedTool: string | null;
}

/**
 * Lightweight obligation detection using Claude Haiku.
 * Intended for Tier 1/2 routed messages where signal detection has already
 * flagged potential obligations. Returns null if no obligation is found.
 * Never throws — returns null on any error.
 */
export async function detectObligationLightweight(
  input: LightweightDetectionInput,
): Promise<LightweightDetectionResult | null> {
  const key = input.gatewayKey ?? process.env["VERCEL_GATEWAY_KEY"] ?? "";
  if (!key) {
    return null;
  }

  const prompt = `You are analyzing a message and a tool response to identify if there is a clear obligation or follow-up action item.

Channel: ${input.channel}
${input.routedTool ? `Handled by tool: ${input.routedTool}` : ""}
Detected signals: ${input.signalResult.signals.join(", ")}

User message:
${input.userMessage}

Tool response:
${input.toolResponse}

Task: Is there a clear action item, commitment, or follow-up obligation? If yes, return a JSON object. If no, return null.

If an obligation exists, return:
{
  "detectedAction": "<imperative verb phrase>",
  "owner": "nova" or "leo",
  "priority": 1, 2, or 3,
  "projectCode": "<string or null>",
  "deadline": "<ISO 8601 or null>"
}

Respond with ONLY the JSON object or the literal null. No other text.`;

  try {
    const anthropic = createClient(key);

    const completion = await anthropic.messages.create({
      model: "claude-haiku-3-5",
      max_tokens: 256,
      messages: [{ role: "user", content: prompt }],
    });

    const block = completion.content[0];
    if (!block || block.type !== "text") {
      return null;
    }

    const trimmed = block.text.trim();
    if (trimmed === "null") {
      return null;
    }

    const raw = extractJsonArray(trimmed);
    const parsed: unknown = JSON.parse(raw);

    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return null;
    }

    const rawOb = parsed as RawObligation;
    const obligation = parseRawObligation(rawOb);
    if (!obligation) {
      return null;
    }

    return {
      ...obligation,
      detectionSource: input.detectionSource,
      routedTool: input.routedTool ?? null,
    };
  } catch {
    return null;
  }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Analyzes a user message and Nova's response to detect any obligations.
 * Returns [] on any error — never throws.
 *
 * @param message    - The user's original message text
 * @param response   - Nova's response text
 * @param channel    - The channel identifier (e.g. "telegram", "dashboard")
 * @param gatewayKey - Optional Vercel AI Gateway key; falls back to env var
 * @param logger     - Optional pino logger; when provided, warnings are logged on
 *                     catch instead of silently swallowed
 */
export async function detectObligations(
  message: string,
  response: string,
  channel: string,
  gatewayKey?: string,
  logger?: Logger,
): Promise<DetectedObligation[]> {
  const key = gatewayKey ?? process.env["VERCEL_GATEWAY_KEY"] ?? "";
  if (!key) {
    return [];
  }

  try {
    const anthropic = createClient(key);
    const prompt = buildDetectionPrompt(message, response, channel);

    const completion = await anthropic.messages.create({
      model: "claude-haiku-3-5",
      max_tokens: 512,
      messages: [{ role: "user", content: prompt }],
    });

    const block = completion.content[0];
    if (!block || block.type !== "text") {
      return [];
    }

    const raw = extractJsonArray(block.text);
    const parsed: unknown = JSON.parse(raw);

    if (!Array.isArray(parsed)) {
      return [];
    }

    const results: DetectedObligation[] = [];
    for (const item of parsed as RawObligation[]) {
      if (typeof item !== "object" || item === null) continue;
      const obligation = parseRawObligation(item);
      if (obligation) {
        results.push(obligation);
      }
    }

    return results;
  } catch (err) {
    // Graceful degradation: return [] so callers are never blocked.
    // Log at warn (not error) because returning [] is an acceptable fallback,
    // not a crash — but the error must be visible for debugging.
    logger?.warn(
      { err, channel },
      "detectObligations: API call failed — returning empty obligations array",
    );
    return [];
  }
}
