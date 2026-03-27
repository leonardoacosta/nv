import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
export async function startMcpServer(registry, logger) {
    const server = new McpServer({
        name: "channels-svc",
        version: "0.1.0",
    });
    // list_channels tool
    server.registerTool("list_channels", {
        description: "List available messaging channels (Telegram, Discord, Teams, etc.) with connection status.",
        inputSchema: z.object({}),
    }, async () => {
        const channels = registry.list();
        return {
            content: [
                { type: "text", text: JSON.stringify(channels, null, 2) },
            ],
        };
    });
    // send_to_channel tool
    server.registerTool("send_to_channel", {
        description: "Send a message to a specific channel. Requires operator confirmation for outbound messages.",
        inputSchema: z.object({
            channel: z.string().describe("Channel name"),
            target: z
                .string()
                .describe("Target identifier (chat ID, channel ID, email address)"),
            message: z.string().describe("Message body"),
        }),
    }, async ({ channel, target, message }) => {
        const channelName = channel;
        const adapter = registry.get(channelName);
        if (!adapter) {
            return {
                content: [
                    {
                        type: "text",
                        text: JSON.stringify({ ok: false, error: `Channel not found: ${channel}` }),
                    },
                ],
                isError: true,
            };
        }
        if (adapter.direction === "inbound") {
            return {
                content: [
                    {
                        type: "text",
                        text: JSON.stringify({
                            ok: false,
                            error: `Channel ${channel} does not support outbound messages`,
                        }),
                    },
                ],
                isError: true,
            };
        }
        const status = adapter.status();
        if (status !== "connected") {
            return {
                content: [
                    {
                        type: "text",
                        text: JSON.stringify({ ok: false, error: `Channel ${channel} is ${status}` }),
                    },
                ],
                isError: true,
            };
        }
        try {
            await adapter.send(target, message);
            logger.info({ channel, target: target.slice(0, 20) }, "MCP: Message sent");
            return {
                content: [
                    {
                        type: "text",
                        text: JSON.stringify({ ok: true, channel, target }),
                    },
                ],
            };
        }
        catch (err) {
            const errorMessage = err instanceof Error ? err.message : "Unknown error";
            logger.error({ channel, target: target.slice(0, 20), error: errorMessage }, "MCP: Failed to send message");
            return {
                content: [
                    {
                        type: "text",
                        text: JSON.stringify({ ok: false, error: errorMessage }),
                    },
                ],
                isError: true,
            };
        }
    });
    const transport = new StdioServerTransport();
    await server.connect(transport);
    logger.info({ service: "channels-svc", transport: "mcp" }, "MCP stdio server started");
}
//# sourceMappingURL=mcp.js.map