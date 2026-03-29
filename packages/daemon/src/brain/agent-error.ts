// ─── AgentErrorCategory ───────────────────────────────────────────────────────

export enum AgentErrorCategory {
  AUTH_FAILURE = "AUTH_FAILURE",
  RATE_LIMITED = "RATE_LIMITED",
  MODEL_UNAVAILABLE = "MODEL_UNAVAILABLE",
  TIMEOUT = "TIMEOUT",
  BINARY_NOT_FOUND = "BINARY_NOT_FOUND",
  UNKNOWN = "UNKNOWN",
}

// ─── AgentError ────────────────────────────────────────────────────────────────

export class AgentError extends Error {
  readonly category: AgentErrorCategory;

  constructor(category: AgentErrorCategory, message: string, cause?: unknown) {
    super(message, cause !== undefined ? { cause } : undefined);
    this.name = "AgentError";
    this.category = category;
  }
}

// ─── classifyAgentError ────────────────────────────────────────────────────────

/**
 * Inspect an unknown thrown value and return the best-matching AgentErrorCategory.
 * If the value is already an AgentError, its category is returned unchanged.
 */
export function classifyAgentError(err: unknown): AgentErrorCategory {
  // Already typed — return existing category
  if (err instanceof AgentError) {
    return err.category;
  }

  const msg =
    err instanceof Error
      ? err.message
      : typeof err === "string"
      ? err
      : "";

  const lower = msg.toLowerCase();

  // AUTH_FAILURE
  if (
    lower.includes("401") ||
    lower.includes("authentication_error") ||
    lower.includes("unauthorized") ||
    lower.includes("invalid api key") ||
    lower.includes("invalid_api_key")
  ) {
    return AgentErrorCategory.AUTH_FAILURE;
  }

  // RATE_LIMITED
  if (
    lower.includes("429") ||
    lower.includes("rate_limit") ||
    lower.includes("rate limited") ||
    lower.includes("too many requests")
  ) {
    return AgentErrorCategory.RATE_LIMITED;
  }

  // MODEL_UNAVAILABLE
  if (
    lower.includes("529") ||
    lower.includes("overloaded") ||
    lower.includes("model_unavailable")
  ) {
    return AgentErrorCategory.MODEL_UNAVAILABLE;
  }

  // TIMEOUT
  if (
    msg.includes("ETIMEDOUT") ||
    msg.includes("ECONNABORTED") ||
    lower.includes("timed out") ||
    lower.includes("timeout")
  ) {
    return AgentErrorCategory.TIMEOUT;
  }

  // BINARY_NOT_FOUND — ENOENT on a path containing "claude"
  if (msg.includes("ENOENT") && lower.includes("claude")) {
    return AgentErrorCategory.BINARY_NOT_FOUND;
  }

  return AgentErrorCategory.UNKNOWN;
}

// ─── USER_MESSAGES ────────────────────────────────────────────────────────────

export const USER_MESSAGES: Record<AgentErrorCategory, string> = {
  [AgentErrorCategory.AUTH_FAILURE]:
    "Nova cannot reach the AI gateway — authentication failed. Check the gateway key configuration.",
  [AgentErrorCategory.RATE_LIMITED]:
    "Nova is rate-limited. Please try again in 30 seconds.",
  [AgentErrorCategory.MODEL_UNAVAILABLE]:
    "The model is temporarily overloaded. Please try again in a moment.",
  [AgentErrorCategory.TIMEOUT]:
    "Nova timed out waiting for a response. Please try again.",
  [AgentErrorCategory.BINARY_NOT_FOUND]:
    "The Claude binary is missing. Nova cannot process messages until it is reinstalled.",
  [AgentErrorCategory.UNKNOWN]:
    "Sorry, something went wrong processing your message. Please try again.",
};
