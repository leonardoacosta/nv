import { readFile } from "node:fs/promises";
import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { Config } from "../config.js";
import type { Message } from "../types.js";
import { logger } from "../logger.js";
import type { AgentResponse, ToolCall } from "./types.js";

const ALLOWED_TOOLS = [
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

  private constructor(config: Config) {
    this.config = config;
  }

  /**
   * Factory — loads system prompt asynchronously before returning the instance.
   */
  static async create(config: Config): Promise<NovaAgent> {
    const agent = new NovaAgent(config);
    await agent.loadSystemPrompt();
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
    _history: Message[],
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

    const queryStream = query({
      prompt: message.content,
      options: {
        systemPrompt: this.systemPrompt,
        allowedTools: ALLOWED_TOOLS,
        permissionMode: "bypassPermissions",
        allowDangerouslySkipPermissions: true,
        maxTurns: 30,
        env: {
          ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
          ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
        },
      },
    });

    for await (const message of queryStream as AsyncIterable<SDKMessage>) {
      if (message.type === "assistant") {
        const content = message.message.content;
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
      } else if (message.type === "result") {
        if (message.subtype === "success") {
          resultText = message.result;
          stopReason = message.stop_reason ?? "end_turn";
        } else {
          throw new Error(
            `Agent query failed: ${message.subtype}`,
          );
        }
      }
    }

    return {
      text: resultText,
      toolCalls,
      stopReason,
    };
  }
}
