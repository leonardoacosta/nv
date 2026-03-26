import Anthropic from "@anthropic-ai/sdk";

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

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Analyzes a user message and Nova's response to detect any obligations.
 * Returns [] on any error — never throws.
 */
export async function detectObligations(
  message: string,
  response: string,
  channel: string,
  gatewayKey?: string,
): Promise<DetectedObligation[]> {
  const key = gatewayKey ?? process.env["VERCEL_GATEWAY_KEY"] ?? "";
  if (!key) {
    return [];
  }

  try {
    const anthropic = createClient(key);
    const prompt = buildDetectionPrompt(message, response, channel);

    const completion = await anthropic.messages.create({
      model: "claude-opus-4-5",
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
  } catch {
    // Any parse failure, network error, or API error returns []
    return [];
  }
}
