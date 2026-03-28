import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import { streamSSE } from "hono/streaming";

import type { NovaAgent } from "./brain/agent.js";
import type { ConversationManager } from "./brain/conversation.js";
import type { Config } from "./config.js";
import type { Message } from "./types.js";
import type { Logger } from "./logger.js";
import type { BriefingDeps } from "./features/briefing/synthesizer.js";
import { gatherContext, synthesizeBriefing, blocksToMarkdown } from "./features/briefing/synthesizer.js";
import { runMorningBriefing } from "./features/briefing/runner.js";
import { runDream, getDreamStatus } from "./features/dream/index.js";

const startedAt = Date.now();

export type ChannelAdapterStatus =
  | "connected"
  | "configured"
  | "disconnected"
  | "unconfigured";

export interface ChannelRegistryEntry {
  name: string;
  status: ChannelAdapterStatus;
  direction: "bidirectional" | "inbound" | "outbound";
}

export interface HttpServerDeps {
  agent: NovaAgent;
  conversationManager: ConversationManager;
  config: Config;
  logger: Logger;
  briefingDeps?: BriefingDeps;
  channelRegistry?: ChannelRegistryEntry[];
}

export function createHttpApp(deps: HttpServerDeps): Hono {
  const { agent, conversationManager, config, logger } = deps;

  const app = new Hono();

  // Middleware
  app.use("*", cors({ origin: "*" }));
  app.use("*", secureHeaders());

  // Global error handler
  app.onError((err, c) => {
    logger.error({ err }, "Unhandled HTTP error");
    return c.json(
      { error: err instanceof Error ? err.message : "Internal Server Error" },
      500,
    );
  });

  // ── GET /health ────────────────────────────────────────────────────────────
  app.get("/health", (c) => {
    return c.json({
      status: "ok",
      service: "nova-daemon",
      uptime_secs: Math.floor((Date.now() - startedAt) / 1000),
    });
  });

  // ── GET /channels/status ───────────────────────────────────────────────────
  app.get("/channels/status", (c) => {
    const registry = deps.channelRegistry ?? [];
    return c.json(registry);
  });

  // ── POST /briefing/generate ─────────────────────────────────────────────────
  app.post("/briefing/generate", async (c) => {
    if (!deps.briefingDeps) {
      return c.json({ error: "Briefing system not configured" }, 503);
    }

    try {
      const row = await runMorningBriefing(deps.briefingDeps);
      return c.json({ id: row.id, generated_at: row.generated_at.toISOString() });
    } catch (err) {
      logger.error({ err }, "POST /briefing/generate failed");
      return c.json(
        { error: err instanceof Error ? err.message : "Briefing generation failed" },
        500,
      );
    }
  });

  // ── GET /api/briefing/stream ────────────────────────────────────────────────
  app.get("/api/briefing/stream", (c) => {
    if (!deps.briefingDeps) {
      return c.json({ error: "Briefing system not configured" }, 503);
    }

    const briefingDeps = deps.briefingDeps;

    return streamSSE(c, async (stream) => {
      try {
        const context = await gatherContext(briefingDeps);
        const synthesis = await synthesizeBriefing(context, briefingDeps);

        const blocks = synthesis.blocks ?? [];

        // Stream individual blocks
        for (let i = 0; i < blocks.length; i++) {
          const block = blocks[i];
          await stream.writeSSE({
            data: JSON.stringify({ type: "block", index: i, block }),
          });
        }

        // Persist to DB
        const pool = briefingDeps.pool;
        const result = await pool.query<{ id: string; generated_at: Date }>(
          `INSERT INTO briefings (content, sources_status, suggested_actions, blocks)
           VALUES ($1, $2, $3, $4)
           RETURNING id, generated_at`,
          [
            synthesis.content,
            JSON.stringify(context.sourcesStatus),
            JSON.stringify(synthesis.suggestedActions),
            synthesis.blocks !== null ? JSON.stringify(synthesis.blocks) : null,
          ],
        );

        const row = result.rows[0];

        // Send done event with full block array
        await stream.writeSSE({
          data: JSON.stringify({ type: "done", blocks }),
        });

        // Send Telegram notification (fire-and-forget)
        if (row && briefingDeps.telegram && briefingDeps.telegramChatId) {
          const TELEGRAM_MAX_LEN = 4096;
          const DASHBOARD_SUFFIX = "\n\n... [view full briefing on dashboard]";
          const content =
            synthesis.content.length <= TELEGRAM_MAX_LEN
              ? synthesis.content
              : synthesis.content.slice(0, TELEGRAM_MAX_LEN - DASHBOARD_SUFFIX.length) + DASHBOARD_SUFFIX;

          void briefingDeps.telegram
            .sendMessage(briefingDeps.telegramChatId, content, {
              parseMode: "Markdown",
              disablePreview: true,
            })
            .catch((err: unknown) => {
              logger.warn({ err }, "Briefing stream: failed to send Telegram notification");
            });
        }
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : "Briefing generation failed";
        logger.error({ err }, "GET /api/briefing/stream error");
        await stream.writeSSE({
          data: JSON.stringify({ type: "error", message }),
        });
      }
    });
  });

  // ── POST /chat ─────────────────────────────────────────────────────────────
  app.post("/chat", async (c) => {
    const body = await c.req.json<{ message?: string }>();

    if (!body.message || typeof body.message !== "string") {
      return c.json({ error: "Missing required field: message" }, 400);
    }

    const userMessage = body.message;

    // Log request
    logger.info(
      {
        service: "nova-daemon",
        route: "POST /chat",
        contentLength: userMessage.length,
        contentPreview: userMessage.slice(0, 80),
      },
      "Chat request received",
    );

    // Construct a Message object for the dashboard channel
    const msg: Message = {
      id: crypto.randomUUID(),
      channel: "dashboard",
      chatId: "dashboard:web",
      text: userMessage,
      content: userMessage,
      type: "text",
      from: {
        id: "dashboard-user",
        firstName: "Dashboard",
      },
      senderId: "dashboard-user",
      senderName: "Dashboard User",
      timestamp: new Date(),
      receivedAt: new Date(),
      metadata: {},
    };

    // Load conversation history
    const history = await conversationManager.loadHistory(
      "dashboard:web",
      config.conversationHistoryDepth,
    );

    const requestStartMs = Date.now();

    return streamSSE(c, async (stream) => {
      // Overall timeout: 120s
      const overallTimeout = setTimeout(() => {
        void stream.writeSSE({
          data: JSON.stringify({
            type: "error",
            message: "Request timed out after 120 seconds",
          }),
        });
        void stream.close();
      }, 120_000);

      // Inactivity timeout: 30s between chunks
      let inactivityTimer: ReturnType<typeof setTimeout> | null = null;

      const resetInactivityTimer = (): void => {
        if (inactivityTimer) clearTimeout(inactivityTimer);
        inactivityTimer = setTimeout(() => {
          void stream.writeSSE({
            data: JSON.stringify({
              type: "error",
              message: "No response received for 30 seconds",
            }),
          });
          void stream.close();
        }, 30_000);
      };

      resetInactivityTimer();

      let fullText = "";
      let toolCallCount = 0;
      let stopReason = "end_turn";

      try {
        for await (const event of agent.processMessageStream(msg, history)) {
          if (event.type === "text_delta") {
            resetInactivityTimer();
            fullText += event.text;
            // Keep wire format as "chunk" for backward compatibility with dashboard
            await stream.writeSSE({
              data: JSON.stringify({ type: "chunk", text: event.text }),
            });
          } else if (event.type === "done") {
            fullText = event.response.text;
            toolCallCount = event.response.toolCalls.length;
            stopReason = event.response.stopReason;
            await stream.writeSSE({
              data: JSON.stringify({
                type: "done",
                full_text: event.response.text,
              }),
            });
          }
          // tool_start and tool_done are ignored in SSE output for now
        }

        // Save exchange -- fire-and-forget
        void conversationManager
          .saveExchange("dashboard:web", msg, {
            ...msg,
            senderId: "nova",
            senderName: "nova",
            content: fullText,
            text: fullText,
          })
          .catch((saveErr: unknown) => {
            logger.warn(
              { service: "nova-daemon", err: saveErr },
              "Failed to save dashboard conversation exchange",
            );
          });

        // Log completion
        const latencyMs = Date.now() - requestStartMs;
        logger.info(
          {
            service: "nova-daemon",
            route: "POST /chat",
            stopReason,
            toolCallCount,
            latencyMs,
          },
          "Chat request completed",
        );
      } catch (err: unknown) {
        const errorMessage =
          err instanceof Error ? err.message : "Agent processing failed";
        logger.error(
          { service: "nova-daemon", route: "POST /chat", err },
          "Chat stream error",
        );
        await stream.writeSSE({
          data: JSON.stringify({ type: "error", message: errorMessage }),
        });
      } finally {
        clearTimeout(overallTimeout);
        if (inactivityTimer) clearTimeout(inactivityTimer);
      }
    });
  });

  // ── POST /dream ──────────────────────────────────────────────────────────────
  app.post("/dream", async (c) => {
    const dryRun = c.req.query("dry_run") === "true";
    const topicMaxKb = config.dream.topicMaxKb;

    try {
      const result = await runDream({ topicMaxKb, dryRun });
      return c.json(result);
    } catch (err) {
      logger.error({ err }, "POST /dream failed");
      return c.json(
        { error: err instanceof Error ? err.message : "Dream cycle failed" },
        500,
      );
    }
  });

  // ── GET /dream/status ───────────────────────────────────────────────────────
  app.get("/dream/status", async (c) => {
    try {
      const status = await getDreamStatus();
      return c.json(status);
    } catch (err) {
      logger.error({ err }, "GET /dream/status failed");
      return c.json(
        { error: err instanceof Error ? err.message : "Failed to get dream status" },
        500,
      );
    }
  });

  return app;
}
