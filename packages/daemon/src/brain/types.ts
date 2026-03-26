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
