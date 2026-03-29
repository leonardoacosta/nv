import { readFile } from "node:fs/promises";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Config } from "../config.js";
import type { Message } from "../types.js";
import { logger } from "../logger.js";
import type { AgentResponse, StreamEvent, ToolCall } from "./types.js";
import { writeEntry } from "../features/diary/index.js";
import { buildToolCallDetail } from "../features/diary/writer.js";
import { estimateCost } from "../features/diary/pricing.js";
import { buildMcpServers, buildAllowedTools } from "./mcp-config.js";
import { createAgentQueryStream } from "./query-factory.js";
import type { DreamScheduler } from "../features/dream/scheduler.js";

/**
 * Format an array of messages into a `<conversation_history>` block
 * suitable for injection into the system prompt.
 */
export function formatHistoryBlock(messages: Message[]): string {
  if (messages.length === 0) return "";

  const lines = messages.map((msg) => {
    const sender = msg.senderId === "nova" ? "nova" : "user";
    const ts = msg.timestamp.toISOString().replace("T", " ").slice(0, 16);
    return `[${sender}] (${ts}): ${msg.content}`;
  });

  return `\n\n<conversation_history>\n${lines.join("\n")}\n</conversation_history>`;
}

/** Built-in Agent SDK tools that are always available. */
const BUILTIN_TOOLS = [
  "Read",
  "Write",
  "Bash",
  "Glob",
  "Grep",
  "WebSearch",
  "WebFetch",
];

export class NovaAgent {
  private readonly config: Config;
  private systemPrompt: string = "";
  private readonly mcpServers: Record<string, { command: string; args: string[]; env?: Record<string, string> }>;
  private readonly allowedTools: string[];
  private _dreamScheduler: DreamScheduler | null = null;

  private constructor(config: Config) {
    this.config = config;
    this.mcpServers = buildMcpServers(config);
    this.allowedTools = buildAllowedTools(this.mcpServers, BUILTIN_TOOLS);
  }

  /**
   * Factory — loads system prompt asynchronously before returning the instance.
   */
  static async create(config: Config): Promise<NovaAgent> {
    const agent = new NovaAgent(config);
    await agent.loadSystemPrompt();
    const mcpNames = Object.keys(agent.mcpServers);
    if (mcpNames.length > 0) {
      logger.info({ mcpServers: mcpNames }, "MCP servers configured for agent");
    }
    return agent;
  }

  /** Wire up the dream scheduler for interaction counting. */
  setDreamScheduler(scheduler: DreamScheduler): void {
    this._dreamScheduler = scheduler;
  }

  private async loadSystemPrompt(): Promise<void> {
    try {
      this.systemPrompt = await readFile(this.config.systemPromptPath, "utf-8");
    } catch (err: unknown) {
      const isNotFound =
        err instanceof Error &&
        "code" in err &&
        (err as NodeJS.ErrnoException).code === "ENOENT";
      if (isNotFound) {
        logger.warn(
          { path: this.config.systemPromptPath },
          "System prompt file not found — falling back to empty string",
        );
        this.systemPrompt = "";
      } else {
        throw err;
      }
    }
  }

  async processMessage(
    message: Message,
    history: Message[],
  ): Promise<AgentResponse> {
    const gatewayKey =
      this.config.vercelGatewayKey ?? process.env["VERCEL_GATEWAY_KEY"];

    const toolCalls: ToolCall[] = [];
    let resultText = "";
    let stopReason = "end_turn";
    let tokensIn = 0;
    let tokensOut = 0;
    const startMs = Date.now();

    const historyBlock = formatHistoryBlock(history);
    const systemPromptWithHistory = this.systemPrompt + historyBlock;

    const queryStream = createAgentQueryStream({
      prompt: message.content,
      systemPrompt: systemPromptWithHistory,
      model: this.config.agent.model,
      allowedTools: this.allowedTools,
      maxTurns: this.config.agent.maxTurns,
      mcpServers: this.mcpServers,
      gatewayKey: gatewayKey ?? undefined,
    });

    for await (const sdkMsg of queryStream as AsyncIterable<SDKMessage>) {
      if (sdkMsg.type === "assistant") {
        const content = sdkMsg.message.content;
        for (const block of content) {
          if (block.type === "tool_use") {
            // Tool calls are tracked; results will be in subsequent messages
            toolCalls.push({
              name: block.name,
              input: block.input as Record<string, unknown>,
              result: null,
            });
          }
        }
        // Accumulate token usage from assistant messages
        const usage = sdkMsg.message.usage;
        if (usage) {
          tokensIn += usage.input_tokens ?? 0;
          tokensOut += usage.output_tokens ?? 0;
        }
      } else if (sdkMsg.type === "result") {
        if (sdkMsg.subtype === "success") {
          resultText = sdkMsg.result;
          stopReason = sdkMsg.stop_reason ?? "end_turn";
        } else {
          throw new Error(
            `Agent query failed: ${sdkMsg.subtype}`,
          );
        }
      }
    }

    const responseLatencyMs = Date.now() - startMs;

    const model = this.config.agent.model;
    const costUsd = estimateCost(model, tokensIn, tokensOut) ?? undefined;

    // Write diary entry — fire-and-forget, never disrupts response path
    void writeEntry({
      triggerType: "message",
      triggerSource: message.senderId,
      channel: message.channel,
      slug: message.content.slice(0, 50),
      content: resultText,
      toolsUsed: toolCalls.map((t) =>
        buildToolCallDetail(t.name, t.input ?? {}, null),
      ),
      tokensIn: tokensIn > 0 ? tokensIn : undefined,
      tokensOut: tokensOut > 0 ? tokensOut : undefined,
      responseLatencyMs,
      model,
      costUsd,
    });

    // Increment dream interaction counter
    if (this._dreamScheduler) {
      this._dreamScheduler.incrementInteractionCount();
    }

    return {
      text: resultText,
      toolCalls,
      stopReason,
    };
  }

  /**
   * Streaming variant of processMessage().
   * Yields rich StreamEvent events: text_delta, tool_start, tool_done, done.
   * Reuses the same allowedTools, mcpServers, and systemPrompt as processMessage().
   */
  async *processMessageStream(
    message: Message,
    history: Message[],
  ): AsyncGenerator<StreamEvent> {
    const gatewayKey =
      this.config.vercelGatewayKey ?? process.env["VERCEL_GATEWAY_KEY"];

    const toolCalls: ToolCall[] = [];
    // Parallel array to toolCalls, populated as tool_done events fire
    const toolCallDetails: { name: string; input: Record<string, unknown>; duration_ms: number | null }[] = [];
    let resultText = "";
    let stopReason = "end_turn";
    let tokensIn = 0;
    let tokensOut = 0;
    const startMs = Date.now();

    // Track in-flight tool calls for tool_start/tool_done pairing
    const inflightTools = new Map<string, { name: string; startedAt: number; input: Record<string, unknown> }>();

    const historyBlock = formatHistoryBlock(history);
    const systemPromptWithHistory = this.systemPrompt + historyBlock;

    const queryStream = createAgentQueryStream({
      prompt: message.content,
      systemPrompt: systemPromptWithHistory,
      model: this.config.agent.model,
      allowedTools: this.allowedTools,
      maxTurns: this.config.agent.maxTurns,
      mcpServers: this.mcpServers,
      gatewayKey: gatewayKey ?? undefined,
    });

    for await (const sdkMsg of queryStream as AsyncIterable<SDKMessage>) {
      if (sdkMsg.type === "assistant") {
        // When a new assistant message arrives after tool_use blocks,
        // it means the tools have completed — emit tool_done for each.
        if (inflightTools.size > 0) {
          const now = Date.now();
          for (const [callId, info] of inflightTools) {
            const durationMs = now - info.startedAt;
            toolCallDetails.push({ name: info.name, input: info.input, duration_ms: durationMs });
            yield {
              type: "tool_done",
              name: info.name,
              callId,
              durationMs,
            };
          }
          inflightTools.clear();
        }

        const content = sdkMsg.message.content;
        for (const block of content) {
          if (block.type === "tool_use") {
            const callId = (block as { id?: string }).id ?? `call_${Date.now()}`;
            const blockInput = block.input as Record<string, unknown>;
            toolCalls.push({
              name: block.name,
              input: blockInput,
              result: null,
            });
            inflightTools.set(callId, { name: block.name, startedAt: Date.now(), input: blockInput });
            yield { type: "tool_start", name: block.name, callId };
          } else if (block.type === "text" && block.text) {
            yield { type: "text_delta", text: block.text };
          }
        }
        const usage = sdkMsg.message.usage;
        if (usage) {
          tokensIn += usage.input_tokens ?? 0;
          tokensOut += usage.output_tokens ?? 0;
        }
      } else if (sdkMsg.type === "result") {
        // Resolve any remaining in-flight tools before the final result
        if (inflightTools.size > 0) {
          const now = Date.now();
          for (const [callId, info] of inflightTools) {
            const durationMs = now - info.startedAt;
            toolCallDetails.push({ name: info.name, input: info.input, duration_ms: durationMs });
            yield {
              type: "tool_done",
              name: info.name,
              callId,
              durationMs,
            };
          }
          inflightTools.clear();
        }

        if (sdkMsg.subtype === "success") {
          resultText = sdkMsg.result;
          stopReason = sdkMsg.stop_reason ?? "end_turn";
        } else {
          throw new Error(
            `Agent query failed: ${sdkMsg.subtype}`,
          );
        }
      }
    }

    const responseLatencyMs = Date.now() - startMs;

    const streamModel = this.config.agent.model;
    const streamCostUsd = estimateCost(streamModel, tokensIn, tokensOut) ?? undefined;

    // For tools that never got a tool_done event (e.g. non-streaming path),
    // fall back to the toolCalls array with null duration
    const structuredTools =
      toolCallDetails.length > 0
        ? toolCallDetails.map((d) => buildToolCallDetail(d.name, d.input, d.duration_ms))
        : toolCalls.map((t) => buildToolCallDetail(t.name, t.input ?? {}, null));

    // Write diary entry -- fire-and-forget
    void writeEntry({
      triggerType: "message",
      triggerSource: message.senderId,
      channel: message.channel,
      slug: message.content.slice(0, 50),
      content: resultText,
      toolsUsed: structuredTools,
      tokensIn: tokensIn > 0 ? tokensIn : undefined,
      tokensOut: tokensOut > 0 ? tokensOut : undefined,
      responseLatencyMs,
      model: streamModel,
      costUsd: streamCostUsd,
    });

    yield {
      type: "done",
      response: { text: resultText, toolCalls, stopReason },
    };
  }
}
