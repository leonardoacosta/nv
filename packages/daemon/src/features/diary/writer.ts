import { db, diary } from "@nova/db";
import { logger } from "../../logger.js";

export interface DiaryWriteInput {
  triggerType: string;
  triggerSource: string;
  channel: string;
  slug: string;
  content: string;
  toolsUsed: string[];
  tokensIn?: number;
  tokensOut?: number;
  responseLatencyMs?: number;
  routingTier?: number;
  routingConfidence?: number;
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
      toolsUsed: input.toolsUsed,
      tokensIn: input.tokensIn ?? null,
      tokensOut: input.tokensOut ?? null,
      responseLatencyMs: input.responseLatencyMs ?? null,
      routingTier: input.routingTier ?? null,
      routingConfidence: input.routingConfidence ?? null,
    });
  } catch (err: unknown) {
    logger.error({ err, slug: input.slug }, "Failed to write diary entry — continuing");
  }
}
