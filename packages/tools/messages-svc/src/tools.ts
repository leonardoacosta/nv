import { desc, eq, ilike, and, type SQL } from "drizzle-orm";
import { db, messages, type Message } from "@nova/db";

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
  handler: (input: Record<string, unknown>) => Promise<string>;
}

export class ToolRegistry {
  private tools = new Map<string, ToolDefinition>();

  register(tool: ToolDefinition): void {
    this.tools.set(tool.name, tool);
  }

  get(name: string): ToolDefinition | undefined {
    return this.tools.get(name);
  }

  list(): ToolDefinition[] {
    return Array.from(this.tools.values());
  }

  async execute(name: string, input: Record<string, unknown>): Promise<string> {
    const tool = this.tools.get(name);
    if (!tool) {
      throw new Error(`Unknown tool: ${name}`);
    }
    return tool.handler(input);
  }
}

// --- Tool implementations ---

function clampLimit(raw: unknown, defaultVal: number, max: number): number {
  const n = typeof raw === "number" ? raw : typeof raw === "string" ? parseInt(raw, 10) : defaultVal;
  if (Number.isNaN(n) || n < 1) return defaultVal;
  return Math.min(n, max);
}

export async function getRecentMessages(
  channel?: string,
  limit?: number,
): Promise<Message[]> {
  const effectiveLimit = clampLimit(limit, 20, 100);
  const conditions: SQL[] = [];

  if (channel) {
    conditions.push(eq(messages.channel, channel));
  }

  const results = await db
    .select({
      id: messages.id,
      channel: messages.channel,
      sender: messages.sender,
      content: messages.content,
      metadata: messages.metadata,
      createdAt: messages.createdAt,
    })
    .from(messages)
    .where(conditions.length > 0 ? and(...conditions) : undefined)
    .orderBy(desc(messages.createdAt))
    .limit(effectiveLimit);

  return results as Message[];
}

export async function searchMessages(
  query: string,
  channel?: string,
  limit?: number,
): Promise<Message[]> {
  const effectiveLimit = clampLimit(limit, 10, 50);
  const conditions: SQL[] = [ilike(messages.content, `%${query}%`)];

  if (channel) {
    conditions.push(eq(messages.channel, channel));
  }

  const results = await db
    .select({
      id: messages.id,
      channel: messages.channel,
      sender: messages.sender,
      content: messages.content,
      metadata: messages.metadata,
      createdAt: messages.createdAt,
    })
    .from(messages)
    .where(and(...conditions))
    .orderBy(desc(messages.createdAt))
    .limit(effectiveLimit);

  return results as Message[];
}

// --- Tool definitions (used by both HTTP and MCP) ---

export const getRecentMessagesTool: ToolDefinition = {
  name: "get_recent_messages",
  description:
    "Retrieve the most recent messages, optionally filtered by channel. Returns messages ordered by created_at descending.",
  inputSchema: {
    type: "object",
    properties: {
      channel: {
        type: "string",
        description: "Filter to a specific channel (telegram, discord, teams, etc.)",
      },
      limit: {
        type: "integer",
        description: "Number of messages to return (default: 20, max: 100)",
      },
    },
    additionalProperties: false,
  },
  handler: async (input) => {
    const channel = typeof input["channel"] === "string" ? input["channel"] : undefined;
    const limit = typeof input["limit"] === "number" ? input["limit"] : undefined;
    const results = await getRecentMessages(channel, limit);
    return JSON.stringify(results);
  },
};

export const searchMessagesTool: ToolDefinition = {
  name: "search_messages",
  description:
    "Search messages using ILIKE text matching on content. Optionally filter by channel.",
  inputSchema: {
    type: "object",
    properties: {
      query: {
        type: "string",
        description: "Search query text (required)",
      },
      channel: {
        type: "string",
        description: "Filter results to a specific channel",
      },
      limit: {
        type: "integer",
        description: "Max results to return (default: 10, max: 50)",
      },
    },
    required: ["query"],
    additionalProperties: false,
  },
  handler: async (input) => {
    const query = input["query"];
    if (typeof query !== "string" || query.trim().length === 0) {
      throw new Error("query is required and must be a non-empty string");
    }
    const channel = typeof input["channel"] === "string" ? input["channel"] : undefined;
    const limit = typeof input["limit"] === "number" ? input["limit"] : undefined;
    const results = await searchMessages(query, channel, limit);
    return JSON.stringify(results);
  },
};
