import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import { createLogger } from "./logger.js";
import { probeFleet, summarizeFleet } from "./health.js";
import { runSelfAssessment } from "./self-assess.js";
import { writeSoul } from "./soul.js";
import { runTypecheck, runBuild } from "./code-tools.js";

const logger = createLogger("meta-svc", { destination: process.stderr });

const server = new McpServer({
  name: "meta-svc",
  version: "0.1.0",
});

// check_services — ping all tool fleet services and return their health status
server.registerTool(
  "check_services",
  {
    description:
      "Check the health status of all Nova fleet services. Returns per-service status and latency.",
    inputSchema: z.object({}),
  },
  async () => {
    const services = await probeFleet();
    const summary = summarizeFleet(services);
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify({ services, summary }, null, 2),
        },
      ],
    };
  },
);

// self_assessment_run — run a self-assessment
server.registerTool(
  "self_assessment_run",
  {
    description:
      "Run a self-assessment. Gathers memory, recent messages, and service health to generate observations.",
    inputSchema: z.object({}),
  },
  async () => {
    const result = await runSelfAssessment();
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(result, null, 2),
        },
      ],
    };
  },
);

// update_soul — update Nova's soul document
server.registerTool(
  "update_soul",
  {
    description: "Update Nova's personality/soul file. Always notify the operator when changing this.",
    inputSchema: z.object({
      changes: z
        .string()
        .describe("The full new content for the soul document"),
    }),
  },
  async ({ changes }) => {
    await writeSoul(changes);
    const bytes = Buffer.byteLength(changes, "utf-8");
    return {
      content: [
        {
          type: "text" as const,
          text: `Soul document updated (${bytes} bytes written)`,
        },
      ],
    };
  },
);

// typecheck_project — run pnpm typecheck on a package
server.registerTool(
  "typecheck_project",
  {
    description:
      "Run TypeScript typecheck on a package. Returns errors if any. Use after modifying code to verify changes.",
    inputSchema: z.object({
      package: z
        .string()
        .optional()
        .describe(
          "Package filter for pnpm (e.g. '@nova/daemon'). If omitted, runs typecheck on the whole workspace.",
        ),
    }),
  },
  async ({ package: pkg }) => {
    const result = await runTypecheck(pkg);
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(result, null, 2),
        },
      ],
    };
  },
);

// build_project — run pnpm build on a package
server.registerTool(
  "build_project",
  {
    description:
      "Run build on a package. Returns build output and success status. Use to verify changes compile and bundle correctly.",
    inputSchema: z.object({
      package: z
        .string()
        .optional()
        .describe(
          "Package filter for pnpm (e.g. '@nova/daemon'). If omitted, runs build on the whole workspace.",
        ),
    }),
  },
  async ({ package: pkg }) => {
    const result = await runBuild(pkg);
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(result, null, 2),
        },
      ],
    };
  },
);

// Start MCP stdio transport
const transport = new StdioServerTransport();
await server.connect(transport);
logger.info(
  { service: "meta-svc", transport: "mcp" },
  "MCP stdio server started",
);
