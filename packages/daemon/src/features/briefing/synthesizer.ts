import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Pool } from "pg";
import type { Logger } from "pino";
import type { Config } from "../../config.js";
import { buildMcpServers, buildAllowedTools } from "../../brain/mcp-config.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export type SourceStatus = "ok" | "unavailable" | "empty";

export interface BriefingDeps {
  pool: Pool;
  gatewayKey: string;
  logger: Logger;
  config?: Config;
}

interface ObligationRow {
  id: string;
  detected_action: string;
  owner: string;
  status: string;
  priority: number;
  project_code: string | null;
  deadline: Date | null;
  created_at: Date;
}

interface MemoryRow {
  topic: string;
  content: string;
  updated_at: Date;
}

interface MessageRow {
  id: string;
  channel: string;
  sender: string | null;
  content: string;
  created_at: Date;
}

export interface GatheredContext {
  obligations: ObligationRow[];
  memory: MemoryRow[];
  messages: MessageRow[];
  sourcesStatus: Record<string, SourceStatus>;
}

export interface SuggestedAction {
  label: string;
  url?: string;
}

export interface SynthesisResult {
  content: string;
  suggestedActions: SuggestedAction[];
}

// ─── Timeout helper ───────────────────────────────────────────────────────────

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`Timed out after ${ms}ms`)), ms),
    ),
  ]);
}

// ─── gatherContext ─────────────────────────────────────────────────────────────

const FETCH_TIMEOUT_MS = 10_000;

export async function gatherContext(deps: BriefingDeps): Promise<GatheredContext> {
  const { pool, logger } = deps;

  const [obligationsResult, memoryResult, messagesResult] = await Promise.allSettled([
    withTimeout(
      pool.query<ObligationRow>(
        `SELECT id, detected_action, owner, status, priority, project_code, deadline, created_at
         FROM obligations
         WHERE status IN ('pending', 'in_progress')
         ORDER BY priority ASC, created_at ASC
         LIMIT 20`,
      ),
      FETCH_TIMEOUT_MS,
    ),
    withTimeout(
      pool.query<MemoryRow>(
        `SELECT topic, content, updated_at
         FROM memory
         ORDER BY updated_at DESC
         LIMIT 10`,
      ),
      FETCH_TIMEOUT_MS,
    ),
    withTimeout(
      pool.query<MessageRow>(
        `SELECT id, channel, sender, content, created_at
         FROM messages
         ORDER BY created_at DESC
         LIMIT 20`,
      ),
      FETCH_TIMEOUT_MS,
    ),
  ]);

  const sourcesStatus: Record<string, SourceStatus> = {};

  let obligations: ObligationRow[] = [];
  if (obligationsResult.status === "fulfilled") {
    obligations = obligationsResult.value.rows;
    sourcesStatus["obligations"] = obligations.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: obligationsResult.reason }, "Failed to fetch obligations for briefing");
    sourcesStatus["obligations"] = "unavailable";
  }

  let memory: MemoryRow[] = [];
  if (memoryResult.status === "fulfilled") {
    memory = memoryResult.value.rows;
    sourcesStatus["memory"] = memory.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: memoryResult.reason }, "Failed to fetch memory for briefing");
    sourcesStatus["memory"] = "unavailable";
  }

  let messages: MessageRow[] = [];
  if (messagesResult.status === "fulfilled") {
    messages = messagesResult.value.rows;
    sourcesStatus["messages"] = messages.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: messagesResult.reason }, "Failed to fetch messages for briefing");
    sourcesStatus["messages"] = "unavailable";
  }

  return { obligations, memory, messages, sourcesStatus };
}

// ─── Static fallback ─────────────────────────────────────────────────────────

function buildStaticSummary(context: GatheredContext): SynthesisResult {
  const pending = context.obligations.filter((o) => o.status === "pending").length;
  const inProgress = context.obligations.filter((o) => o.status === "in_progress").length;

  const lines: string[] = [
    "# Morning Briefing",
    "",
    "## Obligations",
    `- Pending: ${pending}`,
    `- In Progress: ${inProgress}`,
    "",
    "## Memory",
    context.memory.length > 0
      ? `- ${context.memory.length} memory entries available`
      : "- No memory entries",
    "",
    "## Recent Activity",
    context.messages.length > 0
      ? `- ${context.messages.length} recent messages`
      : "- No recent messages",
    "",
    "_Note: AI synthesis unavailable — showing summary counts._",
  ];

  return {
    content: lines.join("\n"),
    suggestedActions: [],
  };
}

// ─── synthesizeBriefing ───────────────────────────────────────────────────────

const SYNTHESIS_TIMEOUT_MS = 30_000;

function buildBriefingPrompt(context: GatheredContext): string {
  const sections: string[] = [];

  // Obligations section
  if (context.obligations.length > 0) {
    sections.push("## Active Obligations");
    for (const o of context.obligations) {
      const deadline = o.deadline ? ` (due: ${o.deadline.toISOString().slice(0, 10)})` : "";
      sections.push(`- [${o.status}] ${o.detected_action} (owner: ${o.owner}, priority: ${o.priority})${deadline}`);
    }
  } else {
    sections.push("## Active Obligations\nNone.");
  }

  // Memory section
  if (context.memory.length > 0) {
    sections.push("\n## Memory Entries");
    for (const m of context.memory) {
      sections.push(`### ${m.topic}\n${m.content}`);
    }
  } else {
    sections.push("\n## Memory Entries\nNone.");
  }

  // Recent messages section
  if (context.messages.length > 0) {
    sections.push("\n## Recent Messages");
    for (const msg of context.messages) {
      const sender = msg.sender ?? "unknown";
      const preview = msg.content.slice(0, 120);
      sections.push(`- [${msg.channel}] ${sender}: ${preview}`);
    }
  } else {
    sections.push("\n## Recent Messages\nNone.");
  }

  return sections.join("\n");
}

const BRIEFING_SYSTEM_PROMPT = `You are Nova's morning briefing synthesizer. Your job is to produce a clear, concise morning briefing from the context provided.

Structure your response with these sections:
1. **Obligations** — summarise the active obligations by priority. Highlight anything urgent or overdue.
2. **Memory Highlights** — surface the most relevant memory entries for today's context.
3. **Recent Activity** — briefly summarise recent message activity and any notable patterns.
4. **Suggested Actions** — list 2-5 concrete actions as a JSON array at the very end of your response.

For the Suggested Actions, end your response with a JSON block in this exact format:
\`\`\`json
[{"label":"Action description","url":"optional-url"}]
\`\`\`

Keep the briefing under 500 words. Be direct and actionable.`;

function parseSuggestedActions(text: string): SuggestedAction[] {
  try {
    const match = /```json\s*([\s\S]*?)```/.exec(text);
    if (!match?.[1]) return [];
    const parsed = JSON.parse(match[1]) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (item): item is SuggestedAction =>
          typeof item === "object" &&
          item !== null &&
          "label" in item &&
          typeof (item as { label: unknown }).label === "string",
      )
      .map((item) => ({
        label: item.label,
        url: typeof item.url === "string" ? item.url : undefined,
      }));
  } catch {
    return [];
  }
}

export async function synthesizeBriefing(
  context: GatheredContext,
  deps: BriefingDeps,
): Promise<SynthesisResult> {
  const { logger, gatewayKey, config } = deps;

  const contextPrompt = buildBriefingPrompt(context);

  // Build MCP server config if available — briefing can access fleet tools
  const mcpServers = config ? buildMcpServers(config) : {};
  const allowedTools = buildAllowedTools(mcpServers, []);

  try {
    const result = await withTimeout(
      (async (): Promise<string> => {
        let resultText = "";

        const queryStream = query({
          prompt: contextPrompt,
          options: {
            systemPrompt: BRIEFING_SYSTEM_PROMPT,
            allowedTools,
            permissionMode: "bypassPermissions",
            allowDangerouslySkipPermissions: true,
            maxTurns: 1,
            mcpServers,
            env: {
              ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
              ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
            },
          },
        });

        for await (const sdkMsg of queryStream as AsyncIterable<SDKMessage>) {
          if (sdkMsg.type === "result" && sdkMsg.subtype === "success") {
            resultText = sdkMsg.result;
          }
        }

        return resultText;
      })(),
      SYNTHESIS_TIMEOUT_MS,
    );

    const suggestedActions = parseSuggestedActions(result);

    // Strip the JSON block from the displayed content
    const content = result.replace(/```json[\s\S]*?```/g, "").trim();

    return { content, suggestedActions };
  } catch (err) {
    logger.warn({ err }, "AI synthesis failed — falling back to static summary");
    return buildStaticSummary(context);
  }
}
