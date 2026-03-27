import type { ObligationStatus, CreateObligationInput } from "../../features/obligations/types.js";

// ─── Types ────────────────────────────────────────────────────────────────────

type StoreFn = (input: CreateObligationInput) => Promise<{ id: string }>;

// ─── Parser ──────────────────────────────────────────────────────────────────

/**
 * Parses the text after /obligation.
 * Format: /obligation <action text> [p1|p2|p3]
 * Priority defaults to 2, owner defaults to "nova".
 */
function parseObligationArgs(text: string): { action: string; priority: number } {
  const trimmed = text.trim();

  // Check for trailing priority flag
  const priorityMatch = /\s+p([123])$/i.exec(trimmed);
  if (priorityMatch) {
    const priority = parseInt(priorityMatch[1]!, 10);
    const action = trimmed.slice(0, priorityMatch.index).trim();
    return { action, priority };
  }

  return { action: trimmed, priority: 2 };
}

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Builds a reply for the /obligation command.
 *
 * @param argsText - Everything after "/obligation " (may be empty)
 * @param storeFn  - Function that creates an obligation and returns its id
 */
export async function buildObligationReply(
  argsText: string | undefined,
  storeFn: StoreFn,
  obligationStatus: ObligationStatus,
): Promise<string> {
  if (!argsText || argsText.trim().length === 0) {
    return "Usage: /obligation <action text> [p1|p2|p3]\nExample: /obligation Review Jira backlog p1";
  }

  const { action, priority } = parseObligationArgs(argsText);

  if (action.length === 0) {
    return "Please provide an action description.";
  }

  const record = await storeFn({
    detectedAction: action,
    owner: "nova",
    status: obligationStatus,
    priority,
    projectCode: null,
    sourceChannel: "telegram",
    sourceMessage: argsText,
    deadline: null,
  });

  return `Obligation created (P${priority}): ${action}\nID: ${record.id.slice(0, 8)}`;
}
