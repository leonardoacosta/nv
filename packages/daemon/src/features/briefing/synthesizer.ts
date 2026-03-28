import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Pool } from "pg";
import type { Logger } from "pino";
import type { Config } from "../../config.js";
import { buildMcpServers, buildAllowedTools } from "../../brain/mcp-config.js";
import { getEntriesByDate } from "../diary/reader.js";
import type { DiaryEntryItem } from "../diary/reader.js";
import { BriefingBlocksSchema } from "@nova/db";
import type { BriefingBlock, BriefingBlocks } from "@nova/db";

// ─── Types ────────────────────────────────────────────────────────────────────

export type SourceStatus = "ok" | "unavailable" | "empty";

export interface BriefingDeps {
  pool: Pool;
  gatewayKey: string;
  logger: Logger;
  config?: Config;
  telegram?: import("../../channels/telegram.js").TelegramAdapter | null;
  telegramChatId?: string | null;
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
  calendar: string | null;
  diaryEntries: DiaryEntryItem[];
  sourcesStatus: Record<string, SourceStatus>;
}

export interface SuggestedAction {
  label: string;
  url?: string;
}

export interface SynthesisResult {
  content: string;
  suggestedActions: SuggestedAction[];
  blocks: BriefingBlock[] | null;
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

  // Yesterday's date for diary entries (overnight activity)
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  const yesterdayStr = yesterday.toISOString().slice(0, 10);

  const [obligationsResult, memoryResult, messagesResult, calendarResult, diaryResult] = await Promise.allSettled([
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
    withTimeout(
      (async (): Promise<string> => {
        const res = await fetch("http://localhost:4107/calendar/today");
        if (!res.ok) throw new Error(`graph-svc returned ${res.status}`);
        const data = (await res.json()) as { result: string };
        return data.result;
      })(),
      FETCH_TIMEOUT_MS,
    ),
    withTimeout(
      getEntriesByDate(yesterdayStr),
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

  let calendar: string | null = null;
  if (calendarResult.status === "fulfilled") {
    calendar = calendarResult.value;
    sourcesStatus["calendar"] = calendar ? "ok" : "empty";
  } else {
    logger.warn({ err: calendarResult.reason }, "Failed to fetch calendar for briefing");
    sourcesStatus["calendar"] = "unavailable";
  }

  let diaryEntries: DiaryEntryItem[] = [];
  if (diaryResult.status === "fulfilled") {
    diaryEntries = diaryResult.value;
    sourcesStatus["diary"] = diaryEntries.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: diaryResult.reason }, "Failed to fetch diary entries for briefing");
    sourcesStatus["diary"] = "unavailable";
  }

  return { obligations, memory, messages, calendar, diaryEntries, sourcesStatus };
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
    blocks: null,
  };
}

// ─── blocksToMarkdown ─────────────────────────────────────────────────────────

/**
 * Converts a validated BriefingBlock[] to a markdown string suitable for
 * Telegram delivery.
 */
export function blocksToMarkdown(blocks: BriefingBlocks): string {
  const parts: string[] = [];

  for (const block of blocks) {
    if (block.title) {
      parts.push(`### ${block.title}`);
    }

    switch (block.type) {
      case "section": {
        parts.push(block.data.body);
        break;
      }

      case "status_table": {
        const { columns, rows } = block.data;
        if (columns.length > 0) {
          // Header row
          parts.push(`| ${columns.join(" | ")} |`);
          parts.push(`| ${columns.map(() => "---").join(" | ")} |`);
          // Data rows
          for (const row of rows) {
            const cells = columns.map((col) => row[col] ?? "");
            parts.push(`| ${cells.join(" | ")} |`);
          }
        }
        break;
      }

      case "metric_card": {
        const { label, value, unit, trend, delta } = block.data;
        const unitStr = unit ? ` ${unit}` : "";
        const trendStr = trend === "up" ? " ^" : trend === "down" ? " v" : "";
        const deltaStr = delta ? ` (${delta})` : "";
        parts.push(`**${label}:** ${value}${unitStr}${trendStr}${deltaStr}`);
        break;
      }

      case "timeline": {
        for (const event of block.data.events) {
          const severityPrefix =
            event.severity === "error" ? "[ERROR] " :
            event.severity === "warning" ? "[WARN] " : "";
          const detail = event.detail ? ` — ${event.detail}` : "";
          parts.push(`- \`${event.time}\` ${severityPrefix}${event.label}${detail}`);
        }
        break;
      }

      case "action_group": {
        for (const action of block.data.actions) {
          const status = action.status ? ` [${action.status}]` : "";
          const url = action.url ? ` (${action.url})` : "";
          parts.push(`- ${action.label}${status}${url}`);
        }
        break;
      }

      case "kv_list": {
        for (const item of block.data.items) {
          parts.push(`**${item.key}:** ${item.value}`);
        }
        break;
      }

      case "alert": {
        const prefix =
          block.data.severity === "error" ? "ERROR" :
          block.data.severity === "warning" ? "WARNING" : "INFO";
        parts.push(`> [${prefix}] ${block.data.message}`);
        break;
      }

      case "source_pills": {
        const pills = block.data.sources.map((s) => {
          const dot = s.status === "ok" ? "✓" : s.status === "unavailable" ? "✗" : "~";
          return `${dot} ${s.name}`;
        });
        parts.push(pills.join("  "));
        break;
      }

      case "pr_list": {
        for (const pr of block.data.prs) {
          const url = pr.url ? ` — ${pr.url}` : "";
          parts.push(`- [${pr.status}] \`${pr.repo}\` ${pr.title}${url}`);
        }
        break;
      }

      case "pipeline_table": {
        for (const pipeline of block.data.pipelines) {
          const duration = pipeline.duration ? ` (${pipeline.duration})` : "";
          parts.push(`- [${pipeline.status}] ${pipeline.name}${duration}`);
        }
        break;
      }
    }

    parts.push(""); // blank line between blocks
  }

  return parts.join("\n").trim();
}

// ─── synthesizeBriefing ───────────────────────────────────────────────────────

const SYNTHESIS_TIMEOUT_MS = 30_000;

function buildBriefingPrompt(context: GatheredContext): string {
  const sections: string[] = [];

  // Messages section — grouped by channel
  if (context.messages.length > 0) {
    sections.push("## Recent Messages");
    const byChannel = new Map<string, MessageRow[]>();
    for (const msg of context.messages) {
      const ch = msg.channel || "unknown";
      if (!byChannel.has(ch)) byChannel.set(ch, []);
      byChannel.get(ch)!.push(msg);
    }
    for (const [channel, msgs] of byChannel) {
      sections.push(`\n### ${channel} (${msgs.length} messages)`);
      for (const msg of msgs) {
        const sender = msg.sender ?? "unknown";
        const preview = msg.content.slice(0, 120);
        sections.push(`- ${sender}: ${preview}`);
      }
    }
  } else {
    sections.push("## Recent Messages\nNone.");
  }

  // Obligations section
  if (context.obligations.length > 0) {
    sections.push("\n## Active Obligations");
    for (const o of context.obligations) {
      const deadline = o.deadline ? ` (due: ${o.deadline.toISOString().slice(0, 10)})` : "";
      sections.push(`- [${o.status}] ${o.detected_action} (owner: ${o.owner}, priority: ${o.priority})${deadline}`);
    }
  } else {
    sections.push("\n## Active Obligations\nNone.");
  }

  // Calendar section
  if (context.calendar) {
    sections.push("\n## Today's Calendar");
    sections.push(context.calendar);
  } else {
    sections.push("\n## Today's Calendar\nNo calendar data available.");
  }

  // Overnight activity section (diary entries)
  if (context.diaryEntries.length > 0) {
    sections.push("\n## Overnight Activity");
    const channels = new Set(context.diaryEntries.map((e) => e.channel_source));
    const tools = new Set(context.diaryEntries.flatMap((e) => e.tools_called));
    sections.push(`- ${context.diaryEntries.length} interactions across channels: ${[...channels].join(", ")}`);
    if (tools.size > 0) {
      sections.push(`- Tools used: ${[...tools].join(", ")}`);
    }
    for (const entry of context.diaryEntries.slice(0, 10)) {
      sections.push(`- [${entry.channel_source}] ${entry.slug}: ${entry.result_summary.slice(0, 100)}`);
    }
  } else {
    sections.push("\n## Overnight Activity\nNo overnight activity recorded.");
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

  return sections.join("\n");
}

const BRIEFING_SYSTEM_PROMPT = `You are Nova's morning briefing synthesizer. Your job is to produce a structured morning briefing as a JSON array of typed blocks.

Output ONLY a raw JSON array — no markdown code fences, no explanation, no preamble. The response must be valid JSON that can be parsed directly.

Each block has this shape:
{
  "type": "<block_type>",
  "title": "<optional title string>",
  "data": { ... type-specific fields ... }
}

Available block types and their data shapes:

1. "section" — prose text
   data: { "body": "string" }

2. "status_table" — tabular data with named columns
   data: { "columns": ["Col1","Col2"], "rows": [{"Col1":"v1","Col2":"v2"}] }

3. "metric_card" — a single KPI metric
   data: { "label": "string", "value": "string|number", "unit": "string (optional)", "trend": "up|down|flat (optional)", "delta": "string (optional)" }

4. "timeline" — chronological events
   data: { "events": [{ "time": "HH:MM", "label": "string", "detail": "string (optional)", "severity": "info|warning|error (optional)" }] }

5. "action_group" — actionable items
   data: { "actions": [{ "label": "string", "url": "string (optional)", "status": "pending|completed|dismissed (optional)" }] }

6. "kv_list" — key-value pairs
   data: { "items": [{ "key": "string", "value": "string" }] }

7. "alert" — highlighted message with severity
   data: { "severity": "info|warning|error", "message": "string" }

8. "source_pills" — data source availability status
   data: { "sources": [{ "name": "string", "status": "ok|unavailable|empty" }] }

9. "pr_list" — pull request list
   data: { "prs": [{ "title": "string", "repo": "string", "url": "string (optional)", "status": "open|merged|closed" }] }

10. "pipeline_table" — CI/CD pipeline status
    data: { "pipelines": [{ "name": "string", "status": "success|failed|running|pending", "duration": "string (optional)" }] }

Structure your briefing with these blocks (in order):
1. A "section" block titled "Messages" summarising unread/new messages by channel
2. A "status_table" or "timeline" block titled "Obligations" showing active items by priority
3. A "timeline" block titled "Calendar" for today's scheduled events
4. A "section" block titled "Overnight Activity" summarising Nova's overnight interactions
5. A "kv_list" or "section" block titled "Memory Highlights" surfacing relevant memory entries
6. An "action_group" block titled "Suggested Actions" with 2-5 concrete next actions

Be direct and concise. Total content should cover the key information without padding.`;

/**
 * Extract SuggestedAction[] from action_group blocks.
 */
function extractSuggestedActions(blocks: BriefingBlock[]): SuggestedAction[] {
  const actions: SuggestedAction[] = [];
  for (const block of blocks) {
    if (block.type === "action_group") {
      for (const action of block.data.actions) {
        actions.push({
          label: action.label,
          url: action.url,
        });
      }
    }
  }
  return actions;
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

    // Attempt JSON parsing + Zod validation
    try {
      const parsed = JSON.parse(result) as unknown;
      const validated = BriefingBlocksSchema.parse(parsed);
      const suggestedActions = extractSuggestedActions(validated);
      const content = blocksToMarkdown(validated);
      return { content, suggestedActions, blocks: validated };
    } catch (parseErr) {
      logger.warn({ err: parseErr }, "Briefing JSON parse/validation failed — falling back to static summary");
      return buildStaticSummary(context);
    }
  } catch (err) {
    logger.warn({ err }, "AI synthesis failed — falling back to static summary");
    return buildStaticSummary(context);
  }
}
