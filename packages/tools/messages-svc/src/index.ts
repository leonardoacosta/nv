import { serve } from "@hono/node-server";

import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { createHttpApp } from "./http.js";
import { startMcpServer } from "./mcp.js";

const config = loadConfig();
const isMcpMode = process.argv.includes("--mcp");

// In MCP mode, logger must write to stderr to avoid corrupting stdio protocol
const logger = createLogger(config.serviceName, {
  level: config.logLevel,
  ...(isMcpMode ? { destination: process.stderr } : {}),
});

if (isMcpMode) {
  // MCP stdio transport
  await startMcpServer(config, logger);
} else {
  // HTTP transport
  const app = createHttpApp(config);

  const server = serve(
    { fetch: app.fetch, port: config.servicePort },
    (info) => {
      logger.info(
        {
          service: config.serviceName,
          port: info.port,
          transport: "http",
        },
        `${config.serviceName} listening on port ${info.port}`,
      );
    },
  );

  // Graceful shutdown
  const shutdown = () => {
    logger.info("Shutting down...");
    server.close(() => {
      logger.info("Server closed");
      process.exit(0);
    });
  };

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);
}
