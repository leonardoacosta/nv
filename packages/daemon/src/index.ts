import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Pool } from "pg";
import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { initFleetClient } from "./fleet-client.js";
import { TelegramAdapter } from "./channels/telegram.js";
import { ProactiveWatcher, handleWatcherCallback } from "./features/watcher/index.js";
import { startBriefingScheduler } from "./features/briefing/scheduler.js";
import { NovaAgent } from "./brain/agent.js";
import { ConversationManager } from "./brain/conversation.js";
import {
  ObligationStore,
  handleObligationConfirm,
  handleObligationReopen,
  OBLIGATION_CONFIRM_PREFIX,
  OBLIGATION_REOPEN_PREFIX,
} from "./features/obligations/index.js";
import { serve } from "@hono/node-server";
import type { ServerType } from "@hono/node-server";
import { createHttpApp } from "./http.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

function readVersion(): string {
  try {
    const pkgPath = join(__dirname, "..", "package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8")) as {
      version: string;
    };
    return pkg.version;
  } catch {
    return "unknown";
  }
}

export async function main(): Promise<void> {
  const config = await loadConfig();
  const log = createLogger("nova-daemon");

  const version = readVersion();

  log.info(
    {
      service: "nova-daemon",
      version,
      configPath: config.configPath,
      daemonPort: config.daemonPort,
    },
    "Nova daemon starting",
  );

  // ── Fleet client initialization ───────────────────────────────────────────

  initFleetClient(config.toolRouterUrl);

  // ── Database pool ──────────────────────────────────────────────────────────

  const pool = new Pool({ connectionString: config.databaseUrl });

  // ── Telegram adapter ───────────────────────────────────────────────────────

  const telegramToken = process.env["TELEGRAM_BOT_TOKEN"];
  let telegram: TelegramAdapter | null = null;

  if (telegramToken) {
    telegram = new TelegramAdapter(telegramToken);
    log.info({ service: "nova-daemon" }, "Telegram adapter initialised");
  } else {
    log.warn(
      { service: "nova-daemon" },
      "TELEGRAM_BOT_TOKEN not set — Telegram adapter disabled",
    );
  }

  // ── Proactive watcher ──────────────────────────────────────────────────────

  let watcher: ProactiveWatcher | null = null;

  if (config.proactiveWatcher.enabled && telegram !== null) {
    const chatId = config.telegramChatId ?? "";

    if (!chatId) {
      log.warn(
        { service: "nova-daemon" },
        "telegramChatId not set — proactive watcher will not send messages",
      );
    }

    watcher = new ProactiveWatcher(pool, telegram, config.proactiveWatcher, log, chatId);
    watcher.start();

    log.info(
      {
        service: "nova-daemon",
        intervalMinutes: config.proactiveWatcher.intervalMinutes,
        quietStart: config.proactiveWatcher.quietStart,
        quietEnd: config.proactiveWatcher.quietEnd,
      },
      `Proactive watcher started (interval: ${config.proactiveWatcher.intervalMinutes}min, quiet: ${config.proactiveWatcher.quietStart}–${config.proactiveWatcher.quietEnd})`,
    );
  }

  // ── Morning briefing scheduler ─────────────────────────────────────────────

  let stopBriefingScheduler: (() => void) | null = null;

  if (config.vercelGatewayKey) {
    stopBriefingScheduler = startBriefingScheduler({
      pool,
      gatewayKey: config.vercelGatewayKey,
      logger: log,
      config,
    });
    log.info({ service: "nova-daemon" }, "Morning briefing scheduler started");
  } else {
    log.warn(
      { service: "nova-daemon" },
      "VERCEL_GATEWAY_KEY not set — morning briefing scheduler disabled",
    );
  }

  // ── NovaAgent ──────────────────────────────────────────────────────────────

  const agent = await NovaAgent.create(config);
  const obligationStore = new ObligationStore(pool);
  const conversationManager = new ConversationManager(pool);

  log.info({ service: "nova-daemon" }, "NovaAgent ready");

  // ── HTTP server (Hono) ───────────────────────────────────────────────────

  const httpApp = createHttpApp({
    agent,
    conversationManager,
    config,
    logger: log,
  });

  let httpServer: ServerType | null = null;
  httpServer = serve(
    { fetch: httpApp.fetch, port: config.daemonPort },
    () => {
      log.info(
        { service: "nova-daemon", port: config.daemonPort },
        `HTTP server listening on port ${config.daemonPort}`,
      );
    },
  );

  // ── Message routing ────────────────────────────────────────────────────────

  if (telegram !== null) {
    telegram.onMessage((msg) => {
      const data = msg.text ?? "";

      // Route watcher inline keyboard callbacks
      if (data.startsWith("watcher:")) {
        const callbackQueryId = String(
          (msg.metadata as { callbackQueryId?: string } | undefined)
            ?.callbackQueryId ?? "",
        );
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );

        void handleWatcherCallback(
          data,
          pool,
          telegram!,
          messageId,
          msg.chatId,
          callbackQueryId,
        );
        return;
      }

      // Route obligation inline keyboard callbacks
      if (data.startsWith(OBLIGATION_CONFIRM_PREFIX)) {
        const id = data.slice(OBLIGATION_CONFIRM_PREFIX.length);
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );

        void handleObligationConfirm(
          id,
          obligationStore,
          telegram!,
          msg.chatId,
          messageId,
        );
        return;
      }

      if (data.startsWith(OBLIGATION_REOPEN_PREFIX)) {
        const id = data.slice(OBLIGATION_REOPEN_PREFIX.length);
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );

        void handleObligationReopen(
          id,
          obligationStore,
          telegram!,
          msg.chatId,
          messageId,
        );
        return;
      }

      // Route regular messages to the agent loop
      log.info(
        {
          service: "nova-daemon",
          chatId: msg.chatId,
          type: msg.type,
          contentLength: msg.content.length,
          contentPreview: msg.content.slice(0, 80),
        },
        "Message received",
      );

      void (async () => {
        // Send typing indicator every 4s until the agent responds.
        // Telegram's "typing" status expires after ~5s, so we refresh it.
        void telegram!.sendChatAction(msg.chatId, "typing");
        const typingInterval = setInterval(() => {
          void telegram!.sendChatAction(msg.chatId, "typing");
        }, 4000);

        try {
          // Load conversation history for this chat
          const channelKey = `telegram:${msg.chatId}`;
          const history = await conversationManager.loadHistory(
            channelKey,
            config.conversationHistoryDepth,
          );

          const response = await agent.processMessage(msg, history);

          // Save exchange fire-and-forget — never block the response path
          void conversationManager.saveExchange(channelKey, msg, {
            ...msg,
            senderId: "nova",
            senderName: "nova",
            content: response.text,
            text: response.text,
          }).catch((saveErr: unknown) => {
            log.warn(
              { service: "nova-daemon", chatId: msg.chatId, err: saveErr },
              "Failed to save conversation exchange",
            );
          });

          // Try Markdown first (agent responses use **bold**, *italic*, `code`).
          // Fall back to plain text if Telegram rejects the formatting.
          try {
            await telegram!.sendMessage(msg.chatId, response.text, {
              parseMode: "Markdown",
              disablePreview: true,
            });
          } catch {
            // Markdown parse failed — strip formatting and send plain text
            try {
              const plain = response.text
                .replace(/\*\*(.+?)\*\*/g, "$1")   // **bold** → bold
                .replace(/\*(.+?)\*/g, "$1")         // *italic* → italic
                .replace(/`([^`]+)`/g, "$1")          // `code` → code
                .replace(/```[\s\S]*?```/g, (m) =>    // ```block``` → block
                  m.replace(/```\w*\n?/g, "").replace(/```/g, ""),
                );
              await telegram!.sendMessage(msg.chatId, plain);
            } catch (sendErr: unknown) {
              log.warn(
                { service: "nova-daemon", chatId: msg.chatId, err: sendErr },
                "sendMessage failed — both Markdown and plain text rejected",
              );
              await telegram!.sendMessage(
                msg.chatId,
                "(Response could not be delivered — contains unsupported formatting.)",
              );
            }
          }

          log.info(
            {
              service: "nova-daemon",
              chatId: msg.chatId,
              stopReason: response.stopReason,
              toolCalls: response.toolCalls.length,
            },
            "Agent response sent",
          );
        } catch (err: unknown) {
          log.error(
            { service: "nova-daemon", chatId: msg.chatId, err },
            "Agent processing failed",
          );
          void telegram!.sendMessage(msg.chatId, "Sorry, something went wrong.");
        } finally {
          clearInterval(typingInterval);
        }
      })();
    });
  }

  // ── Graceful shutdown ──────────────────────────────────────────────────────

  const shutdown = async (): Promise<void> => {
    log.info({ service: "nova-daemon" }, "Shutting down…");

    if (stopBriefingScheduler !== null) {
      stopBriefingScheduler();
    }

    if (watcher !== null) {
      watcher.stop();
    }

    if (telegram !== null) {
      telegram.stop();
    }

    if (httpServer !== null) {
      httpServer.close();
    }

    await pool.end();
    process.exit(0);
  };

  process.on("SIGTERM", () => { void shutdown(); });
  process.on("SIGINT", () => { void shutdown(); });

  log.info(
    { service: "nova-daemon", toolRouterUrl: config.toolRouterUrl },
    "Nova daemon ready",
  );
}

main().catch((err: unknown) => {
  console.error("Fatal error during startup:", err);
  process.exit(1);
});
