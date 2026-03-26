import type { Hono } from "hono";
import type { Logger } from "pino";

import { getServiceForTool } from "../registry.js";

/**
 * POST /dispatch
 *
 * Accepts { tool, input }, looks up the target service, forwards
 * the request as POST {serviceUrl}/tools/{tool}, and returns the
 * downstream response verbatim.
 *
 * Error codes:
 *   404 — unknown tool (not in registry)
 *   502 — downstream service unreachable or errored
 */
export function dispatchRoute(app: Hono, logger: Logger): void {
  app.post("/dispatch", async (c) => {
    const body = await c.req.json<{ tool?: string; input?: unknown }>().catch(() => null);

    if (!body?.tool) {
      return c.json({ error: "missing_tool", message: "Request body must include 'tool'" }, 400);
    }

    const toolName = body.tool;
    const entry = getServiceForTool(toolName);

    if (!entry) {
      logger.warn({ tool: toolName }, "Dispatch: unknown tool");
      return c.json({ error: "unknown_tool", tool: toolName }, 404);
    }

    const targetUrl = `${entry.serviceUrl}/tools/${toolName}`;
    logger.info({ tool: toolName, service: entry.serviceName, url: targetUrl }, "Dispatching");

    try {
      const resp = await fetch(targetUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body.input ?? {}),
      });

      const respBody = await resp.text();
      // Forward downstream status and body
      return new Response(respBody, {
        status: resp.status,
        headers: { "Content-Type": resp.headers.get("Content-Type") ?? "application/json" },
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Service unavailable";
      logger.error({ tool: toolName, service: entry.serviceName, err }, "Dispatch: service unreachable");
      return c.json(
        { error: "service_unavailable", service: entry.serviceName, tool: toolName, message },
        502,
      );
    }
  });
}
