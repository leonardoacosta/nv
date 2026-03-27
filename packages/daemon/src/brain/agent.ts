import { readFile } from "node:fs/promises";
import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Config } from "../config.js";
import type { Message } from "../types.js";
import { logger } from "../logger.js";
import type { AgentResponse, ToolCall } from "./types.js";
import { writeEntry } from "../features/diary/index.js";
import { buildMcpServers, buildAllowedTools } from "./mcp-config.js";

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

    if (!gatewayKey) {
      throw new Error(
        "Vercel AI Gateway key is required but not configured. " +
          "Set VERCEL_GATEWAY_KEY environment variable or vercelGatewayKey in config.",
      );
    }

    const toolCalls: ToolCall[] = [];
    let resultText = "";
    let stopReason = "end_turn";
    let tokensIn = 0;
    let tokensOut = 0;
    const startMs = Date.now();

    const historyBlock = formatHistoryBlock(history);
    const systemPromptWithHistory = this.systemPrompt + historyBlock;

    const queryStream = query({
      prompt: message.content,
      options: {
        systemPrompt: systemPromptWithHistory,
        allowedTools: this.allowedTools,
        permissionMode: "bypassPermissions",
        allowDangerouslySkipPermissions: true,
        maxTurns: 30,
        mcpServers: this.mcpServers,
        env: {
          ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
          ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
        },
      },
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

    // Write diary entry — fire-and-forget, never disrupts response path
    void writeEntry({
      triggerType: "message",
      triggerSource: message.senderId,
      channel: message.channel,
      slug: message.content.slice(0, 50),
      content: resultText,
      toolsUsed: toolCalls.map((t) => t.name),
      tokensIn: tokensIn > 0 ? tokensIn : undefined,
      tokensOut: tokensOut > 0 ? tokensOut : undefined,
      responseLatencyMs,
    });

    return {
      text: resultText,
      toolCalls,
      stopReason,
    };
  }

  /**
   * Streaming variant of processMessage().
   * Yields `chunk` events as assistant text blocks arrive, then a final `done`
   * event with the full AgentResponse. Reuses the same allowedTools, mcpServers,
   * and systemPrompt as processMessage().
   */
  async *processMessageStream(
    message: Message,
    history: Message[],
  ): AsyncGenerator<
    | { type: "chunk"; text: string }
    | { type: "done"; response: AgentResponse }
  > {
    const gatewayKey =
      this.config.vercelGatewayKey ?? process.env["VERCEL_GATEWAY_KEY"];

    if (!gatewayKey) {
      throw new Error(
        "Vercel AI Gateway key is required but not configured. " +
          "Set VERCEL_GATEWAY_KEY environment variable or vercelGatewayKey in config.",
      );
    }

    const toolCalls: ToolCall[] = [];
    let resultText = "";
    let stopReason = "end_turn";
    let tokensIn = 0;
    let tokensOut = 0;
    const startMs = Date.now();

    const historyBlock = formatHistoryBlock(history);
    const systemPromptWithHistory = this.systemPrompt + historyBlock;

    const queryStream = query({
      prompt: message.content,
      options: {
        systemPrompt: systemPromptWithHistory,
        allowedTools: this.allowedTools,
        permissionMode: "bypassPermissions",
        allowDangerouslySkipPermissions: true,
        maxTurns: 30,
        mcpServers: this.mcpServers,
        env: {
          ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
          ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
        },
      },
    });

    for await (const sdkMsg of queryStream as AsyncIterable<SDKMessage>) {
      if (sdkMsg.type === "assistant") {
        const content = sdkMsg.message.content;
        for (const block of content) {
          if (block.type === "tool_use") {
            toolCalls.push({
              name: block.name,
              input: block.input as Record<string, unknown>,
              result: null,
            });
          } else if (block.type === "text" && block.text) {
            yield { type: "chunk", text: block.text };
          }
        }
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

    // Write diary entry -- fire-and-forget
    void writeEntry({
      triggerType: "message",
      triggerSource: message.senderId,
      channel: message.channel,
      slug: message.content.slice(0, 50),
      content: resultText,
      toolsUsed: toolCalls.map((t) => t.name),
      tokensIn: tokensIn > 0 ? tokensIn : undefined,
      tokensOut: tokensOut > 0 ? tokensOut : undefined,
      responseLatencyMs,
    });

    yield {
      type: "done",
      response: { text: resultText, toolCalls, stopReason },
    };
  }
}
