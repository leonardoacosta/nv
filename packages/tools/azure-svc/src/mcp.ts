import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import type { ServiceConfig } from "./config.js";
import type { Logger } from "./logger.js";
import { runAzureCli } from "./tools/azure-cli.js";

export async function startMcpServer(
  config: ServiceConfig,
  logger: Logger,
): Promise<void> {
  const server = new McpServer({
    name: config.serviceName,
    version: "0.1.0",
  });

  // Single tool: azure_cli
  server.registerTool(
    "azure_cli",
    {
      description:
        "Run any Azure CLI command. Authenticated and ready to use. " +
        "Pass the full command including 'az' prefix " +
        "(e.g. 'az vm list', 'az group list', 'az account show'). " +
        "Returns JSON output by default. All Azure operations are available.",
      inputSchema: z.object({
        command: z
          .string()
          .describe(
            "The full Azure CLI command to run, starting with 'az'. " +
            "Examples: 'az vm list', 'az group list --output table', " +
            "'az account show', 'az resource list --resource-group myRG'.",
          ),
      }),
    },
    async ({ command }) => {
      try {
        const result = await runAzureCli(config, command);
        return {
          content: [{ type: "text" as const, text: result }],
        };
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Unknown error";
        logger.error(
          { command: command.slice(0, 80), error: errorMessage },
          "MCP: azure_cli failed",
        );
        return {
          content: [
            {
              type: "text" as const,
              text: JSON.stringify({ ok: false, error: errorMessage }),
            },
          ],
          isError: true,
        };
      }
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);

  logger.info({ service: config.serviceName, transport: "mcp" }, "MCP stdio server started");
}
