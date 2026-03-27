export interface ToolCall {
  name: string;
  input: Record<string, unknown>;
  result: unknown;
}

export interface AgentResponse {
  text: string;
  toolCalls: ToolCall[];
  stopReason: string;
}

export type StreamEvent =
  | { type: "text_delta"; text: string }
  | { type: "tool_start"; name: string; callId: string }
  | { type: "tool_done"; name: string; callId: string; durationMs: number }
  | { type: "done"; response: AgentResponse };
