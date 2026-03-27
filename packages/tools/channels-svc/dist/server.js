import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
const startedAt = Date.now();
export function createHttpApp(registry, config, logger) {
    const app = new Hono();
    // Middleware
    app.use("*", cors({ origin: config.corsOrigin }));
    app.use("*", secureHeaders());
    // Global error handler
    app.onError((err, c) => {
        logger.error({ err }, "Unhandled error");
        return c.json({ error: err instanceof Error ? err.message : "Internal Server Error" }, 500);
    });
    // Health endpoint
    app.get("/health", (c) => {
        return c.json({
            status: "ok",
            service: config.serviceName,
            port: config.servicePort,
            uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
        });
    });
    // List channels
    app.get("/channels", (c) => {
        return c.json({ channels: registry.list() });
    });
    // Send message
    app.post("/send", async (c) => {
        const body = await c.req.json();
        const { channel, target, message } = body;
        if (!channel || !target || !message) {
            return c.json({ ok: false, error: "Missing required fields: channel, target, message" }, 400);
        }
        const channelName = channel;
        const adapter = registry.get(channelName);
        if (!adapter) {
            return c.json({ ok: false, error: `Channel not found: ${channel}` }, 404);
        }
        // Check direction supports outbound
        if (adapter.direction === "inbound") {
            return c.json({ ok: false, error: `Channel ${channel} does not support outbound messages` }, 400);
        }
        // Check status
        const status = adapter.status();
        if (status !== "connected") {
            return c.json({ ok: false, error: `Channel ${channel} is ${status}` }, 503);
        }
        try {
            await adapter.send(target, message);
            logger.info({ channel, target: target.slice(0, 20) }, "Message sent");
            return c.json({ ok: true, channel, target });
        }
        catch (err) {
            const errorMessage = err instanceof Error ? err.message : "Unknown error";
            logger.error({ channel, target: target.slice(0, 20), error: errorMessage }, "Failed to send message");
            return c.json({ ok: false, error: errorMessage }, 502);
        }
    });
    return app;
}
//# sourceMappingURL=server.js.map