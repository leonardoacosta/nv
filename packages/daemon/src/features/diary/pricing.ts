/**
 * Static cost estimation for Claude API calls.
 *
 * Returns null for unknown models rather than returning incorrect values.
 * Update the pricing table when Anthropic changes rates.
 */

interface ModelPricing {
  inputPerMillion: number;  // USD per 1M input tokens
  outputPerMillion: number; // USD per 1M output tokens
}

const PRICING_TABLE: Record<string, ModelPricing> = {
  "claude-opus-4-6": { inputPerMillion: 15.0, outputPerMillion: 75.0 },
  "claude-sonnet-4-5": { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  "claude-haiku-3-5": { inputPerMillion: 0.8, outputPerMillion: 4.0 },
};

/**
 * Estimate the USD cost of a Claude API call.
 *
 * @param model - Model name (e.g. "claude-opus-4-6")
 * @param tokensIn - Input token count
 * @param tokensOut - Output token count
 * @returns Estimated cost in USD, or null for unknown models
 */
export function estimateCost(
  model: string | undefined | null,
  tokensIn: number,
  tokensOut: number,
): number | null {
  if (!model) return null;

  const pricing = PRICING_TABLE[model];
  if (!pricing) return null;

  const inputCost = (tokensIn / 1_000_000) * pricing.inputPerMillion;
  const outputCost = (tokensOut / 1_000_000) * pricing.outputPerMillion;

  return inputCost + outputCost;
}
