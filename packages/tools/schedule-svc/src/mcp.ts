import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import type { ServiceConfig } from "./config.js";
import type { Logger } from "./logger.js";
import type { ToolRegistry } from "./tool-registry.js";

// Map JSON Schema type definitions to Zod schemas for MCP registration
function buildZodSchema(inputSchema: Record<string, unknown>): z.ZodObject<z.ZodRawShape> {
  const properties = (inputSchema["properties"] ?? {}) as Record<string, Record<string, unknown>>;
  const required = (inputSchema["required"] ?? []) as string[];

  const shape: z.ZodRawShape = {};

  for (const [key, prop] of Object.entries(properties)) {
    let field: z.ZodTypeAny;
    const propType = prop["type"] as string;

    if (propType === "object") {
      // Nested objects: use passthrough for flexible input
      field = z.record(z.unknown()).describe(prop["description"] as string ?? "");
    } else if (propType === "boolean") {
      field = z.boolean();
    } else if (propType === "number" || propType === "integer") {
      field = z.number();
    } else {
      // string or enum
      const enumValues = prop["enum"] as string[] | undefined;
      if (enumValues && enumValues.length > 0) {
        field = z.enum(enumValues as [string, ...string[]]);
      } else {
        field = z.string();
      }
    }

    if (prop["description"]) {
      field = field.describe(prop["description"] as string);
    }

    if (!required.includes(key)) {
      field = field.optional();
    }

    shape[key] = field;
  }

  return z.object(shape);
}

export async function startMcpServer(
  registry: ToolRegistry,
  config: ServiceConfig,
  logger: Logger,
): Promise<void> {
  const server = new McpServer({
    name: config.serviceName,
    version: "0.1.0",
  });

  // Register all tools from the shared registry
  for (const tool of registry.list()) {
    const zodSchema = buildZodSchema(tool.inputSchema);

    server.registerTool(
      tool.name,
      {
        description: tool.description,
        inputSchema: zodSchema,
      },
      async (input) => {
        const result = await tool.handler(input as Record<string, unknown>);
        return {
          content: [{ type: "text" as const, text: result }],
        };
      },
    );
  }

  const transport = new StdioServerTransport();
  await server.connect(transport);

  logger.info({ service: config.serviceName, transport: "mcp" }, "MCP stdio server started");
}
