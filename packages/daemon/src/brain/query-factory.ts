/**
 * Centralized Agent SDK query factory.
 * All callers that need to invoke claude-agent-sdk query() should use
 * createAgentQuery() or createAgentQueryStream() from this module rather than
 * constructing gateway env vars, permission modes, and timeout races inline.
 */

import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type { McpStdioServerConfig } from "./mcp-config.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface AgentQueryOptions {
  prompt: string;
  systemPrompt?: string;
  model?: string;
  maxTurns: number;
  timeoutMs: number;
  mcpServers?: Record<string, McpStdioServerConfig>;
  allowedTools?: string[];
  /** Override VERCEL_GATEWAY_KEY env var. Required if the env var is absent. */
  gatewayKey?: string;
}

export interface AgentQueryResult {
  text: string;
  inputTokens: number;
  outputTokens: number;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function resolveGatewayKey(override?: string): string {
  const key = override ?? process.env["VERCEL_GATEWAY_KEY"];
  if (!key) {
    throw new Error(
      "Vercel AI Gateway key is required but not configured. " +
        "Set VERCEL_GATEWAY_KEY environment variable or pass gatewayKey in options.",
    );
  }
  return key;
}

function buildGatewayEnv(gatewayKey: string): Record<string, string> {
  return {
    ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
    ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
  };
}

function createTimeoutReject(ms: number): Promise<never> {
  return new Promise<never>((_, reject) => {
    setTimeout(() => {
      reject(new Error(`Agent query timed out after ${ms}ms`));
    }, ms);
  });
}

// ─── createAgentQuery ─────────────────────────────────────────────────────────

/**
 * Run a single-shot Agent SDK query.
 * Returns { text, inputTokens, outputTokens } on success.
 * Throws a normalized Error on non-success subtype, missing gateway key, or timeout.
 */
export async function createAgentQuery(
  options: AgentQueryOptions,
): Promise<AgentQueryResult> {
  const gatewayKey = resolveGatewayKey(options.gatewayKey);

  const queryStream = query({
    prompt: options.prompt,
    options: {
      ...(options.systemPrompt !== undefined
        ? { systemPrompt: options.systemPrompt }
        : {}),
      ...(options.model !== undefined ? { model: options.model } : {}),
      allowedTools: options.allowedTools ?? [],
      permissionMode: "bypassPermissions",
      allowDangerouslySkipPermissions: true,
      maxTurns: options.maxTurns,
      ...(options.mcpServers !== undefined
        ? { mcpServers: options.mcpServers }
        : {}),
      env: buildGatewayEnv(gatewayKey),
    },
  });

  let resultText = "";
  let inputTokens = 0;
  let outputTokens = 0;

  const queryPromise = (async () => {
    for await (const message of queryStream as AsyncIterable<SDKMessage>) {
      if (message.type === "result") {
        if (message.subtype === "success") {
          resultText = message.result;
        } else {
          throw new Error(`Agent query failed: ${message.subtype}`);
        }
      }
      if (message.type === "assistant" && message.message?.usage) {
        const usage = message.message.usage as {
          input_tokens?: number;
          output_tokens?: number;
        };
        inputTokens += usage.input_tokens ?? 0;
        outputTokens += usage.output_tokens ?? 0;
      }
    }
    return { text: resultText, inputTokens, outputTokens };
  })();

  return Promise.race([queryPromise, createTimeoutReject(options.timeoutMs)]);
}

// ─── createAgentQueryStream ───────────────────────────────────────────────────

/**
 * Streaming variant of createAgentQuery.
 * Returns the raw AsyncIterable<SDKMessage> so callers can process per-event
 * (tool_start, tool_done, text_delta, etc.) without losing event granularity.
 * Gateway env vars and permission modes are configured identically to
 * createAgentQuery(). Timeout is NOT applied here — callers that need a timeout
 * on the stream should wrap the iteration in Promise.race externally.
 */
export function createAgentQueryStream(
  options: Omit<AgentQueryOptions, "timeoutMs"> & { timeoutMs?: number },
): AsyncIterable<SDKMessage> {
  const gatewayKey = resolveGatewayKey(options.gatewayKey);

  return query({
    prompt: options.prompt,
    options: {
      ...(options.systemPrompt !== undefined
        ? { systemPrompt: options.systemPrompt }
        : {}),
      ...(options.model !== undefined ? { model: options.model } : {}),
      allowedTools: options.allowedTools ?? [],
      permissionMode: "bypassPermissions",
      allowDangerouslySkipPermissions: true,
      maxTurns: options.maxTurns,
      ...(options.mcpServers !== undefined
        ? { mcpServers: options.mcpServers }
        : {}),
      env: buildGatewayEnv(gatewayKey),
    },
  }) as AsyncIterable<SDKMessage>;
}
