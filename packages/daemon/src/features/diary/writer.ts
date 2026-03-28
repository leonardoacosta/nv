import { db, diary } from "@nova/db";
import { logger } from "../../logger.js";

export interface ToolCallDetail {
  name: string;
  input_summary: string;   // first 120 chars of JSON.stringify(input), truncated
  duration_ms: number | null;
}

export interface DiaryWriteInput {
  triggerType: string;
  triggerSource: string;
  channel: string;
  slug: string;
  content: string;
  toolsUsed: Array<ToolCallDetail> | string[];
  tokensIn?: number;
  tokensOut?: number;
  responseLatencyMs?: number;
  routingTier?: number;
  routingConfidence?: number;
  model?: string;
  costUsd?: number;
}

/**
 * Write a diary entry to Postgres via Drizzle.
 * Never throws — errors are logged and suppressed so a diary failure
 * never disrupts the main response path.
 */
export async function writeEntry(input: DiaryWriteInput): Promise<void> {
  try {
    await db.insert(diary).values({
      triggerType: input.triggerType,
      triggerSource: input.triggerSource,
      channel: input.channel,
      slug: input.slug,
      content: input.content,
      toolsUsed: input.toolsUsed as unknown[],
      tokensIn: input.tokensIn ?? null,
      tokensOut: input.tokensOut ?? null,
      responseLatencyMs: input.responseLatencyMs ?? null,
      routingTier: input.routingTier ?? null,
      routingConfidence: input.routingConfidence ?? null,
      model: input.model ?? null,
      costUsd: input.costUsd != null ? String(input.costUsd) : null,
    });
  } catch (err: unknown) {
    logger.error({ err, slug: input.slug }, "Failed to write diary entry — continuing");
  }
}

/**
 * Build a ToolCallDetail from a raw tool call.
 * Safely stringifies the input and truncates to 120 chars.
 */
export function buildToolCallDetail(
  name: string,
  input: Record<string, unknown>,
  duration_ms: number | null,
): ToolCallDetail {
  let input_summary = "";
  try {
    input_summary = JSON.stringify(input).slice(0, 120);
  } catch {
    input_summary = "";
  }
  return { name, input_summary, duration_ms };
}
