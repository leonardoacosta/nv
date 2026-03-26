import OpenAI from "openai";
import type { Logger } from "./logger.js";

let client: OpenAI | null = null;
let warnedMissingKey = false;

function getClient(apiKey: string | undefined, logger: Logger): OpenAI | null {
  if (client) return client;
  if (!apiKey) {
    if (!warnedMissingKey) {
      logger.warn("OPENAI_API_KEY not set — embedding generation disabled, using substring search fallback");
      warnedMissingKey = true;
    }
    return null;
  }
  client = new OpenAI({ apiKey });
  return client;
}

export async function generateEmbedding(
  text: string,
  apiKey: string | undefined,
  logger: Logger,
): Promise<number[] | null> {
  const openai = getClient(apiKey, logger);
  if (!openai) return null;

  try {
    const response = await openai.embeddings.create({
      model: "text-embedding-3-small",
      input: text,
      dimensions: 1536,
    });
    return response.data[0]?.embedding ?? null;
  } catch (err) {
    logger.error({ err }, "Failed to generate embedding — writing without embedding");
    return null;
  }
}
