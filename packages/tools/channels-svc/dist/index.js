import { serve } from "@hono/node-server";
import { createLogger } from "./logger.js";
import { createHttpApp } from "./server.js";
import { startMcpServer } from "./mcp.js";
import { AdapterRegistry } from "./adapters/registry.js";
import { TelegramAdapter } from "./adapters/telegram.js";
import { DiscordAdapter } from "./adapters/discord.js";
import { TeamsAdapter } from "./adapters/teams.js";
import { EmailAdapter } from "./adapters/email.js";
import { IMessageAdapter } from "./adapters/imessage.js";
const SERVICE_NAME = "channels-svc";
const DEFAULT_PORT = 4003;
const isMcpMode = process.argv.includes("--mcp");
const servicePort = parseInt(process.env["PORT"] ?? String(DEFAULT_PORT), 10);
const corsOrigin = process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev";
// In MCP mode, logger must write to stderr to avoid corrupting stdio protocol
const logger = createLogger(SERVICE_NAME, {
    ...(isMcpMode ? { destination: process.stderr } : {}),
});
// Build adapter registry
const registry = new AdapterRegistry();
// Telegram: real adapter
const telegramToken = process.env["TELEGRAM_BOT_TOKEN"];
registry.register(new TelegramAdapter(telegramToken));
// Stub adapters
registry.register(new DiscordAdapter());
registry.register(new TeamsAdapter());
registry.register(new EmailAdapter());
registry.register(new IMessageAdapter());
const adapters = registry.list();
logger.info({
    adapters: adapters.map((a) => ({ name: a.name, status: a.status })),
}, `Registered ${adapters.length} channel adapters`);
if (isMcpMode) {
    // MCP stdio transport
    await startMcpServer(registry, logger);
}
else {
    // HTTP transport
    const app = createHttpApp(registry, { serviceName: SERVICE_NAME, servicePort, corsOrigin }, logger);
    const server = serve({ fetch: app.fetch, port: servicePort }, (info) => {
        logger.info({
            service: SERVICE_NAME,
            port: info.port,
            transport: "http",
        }, `${SERVICE_NAME} listening on port ${info.port}`);
    });
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
//# sourceMappingURL=index.js.map