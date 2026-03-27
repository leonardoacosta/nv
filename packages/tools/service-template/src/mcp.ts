import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

import type { ServiceConfig } from "./config.js";
import type { Logger } from "./logger.js";
import { jsonSchemaToZod } from "./schema-utils.js";
import type { ToolRegistry } from "./tools.js";

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
    const zodSchema = jsonSchemaToZod(tool.inputSchema);
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
