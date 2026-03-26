import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Pool } from "pg";
import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { startApiServer } from "./api/server.js";
import { TelegramAdapter } from "./channels/telegram.js";
import { ProactiveWatcher, handleWatcherCallback } from "./features/watcher/index.js";
import { startBriefingScheduler } from "./features/briefing/scheduler.js";
import { NovaAgent } from "./brain/agent.js";
import {
  ObligationStore,
  handleObligationConfirm,
  handleObligationReopen,
  OBLIGATION_CONFIRM_PREFIX,
  OBLIGATION_REOPEN_PREFIX,
} from "./features/obligations/index.js";

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

  log.info({ service: "nova-daemon" }, "NovaAgent ready");

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
      void (async () => {
        try {
          void telegram!.sendChatAction(msg.chatId, "typing");
          const response = await agent.processMessage(msg, []);
          await telegram!.sendMessage(msg.chatId, response.text);
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

    await pool.end();
    process.exit(0);
  };

  process.on("SIGTERM", () => { void shutdown(); });
  process.on("SIGINT", () => { void shutdown(); });

  // ── API server ─────────────────────────────────────────────────────────────

  const apiPort = Number(process.env["API_PORT"] ?? 3443);
  await startApiServer(apiPort);
  log.info({ service: "nova-daemon", port: apiPort }, `API server listening on :${apiPort}`);

  log.info({ service: "nova-daemon" }, "Nova daemon ready");
}

main().catch((err: unknown) => {
  console.error("Fatal error during startup:", err);
  process.exit(1);
});
