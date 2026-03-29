import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Pool } from "pg";
import { loadConfig } from "./config.js";
import { createLogger } from "./logger.js";
import { initFleetClient } from "./fleet-client.js";
import { TelegramAdapter } from "./channels/telegram.js";
import { TelegramStreamWriter } from "./channels/stream-writer.js";
import { ProactiveWatcher, handleWatcherCallback } from "./features/watcher/index.js";
import { startBriefingScheduler } from "./features/briefing/scheduler.js";
import { DreamScheduler } from "./features/dream/index.js";
import { startDigestScheduler } from "./features/digest/index.js";
import { startTokenRefresh } from "./features/token-refresh.js";
import { setDigestDeps } from "./telegram/commands/digest.js";
import { JobQueue } from "./queue/index.js";
import { ThreadResolver } from "./queue/thread-resolver.js";
import type { EnqueueResult } from "./queue/types.js";
import { NovaAgent } from "./brain/agent.js";
import { ConversationManager } from "./brain/conversation.js";
import { KeywordRouter } from "./brain/keyword-router.js";
import { EmbeddingRouter } from "./brain/embedding-router.js";
import { MessageRouter, formatToolResponse } from "./brain/router.js";
import { fleetPost } from "./fleet-client.js";
import { writeEntry, buildToolCallDetail } from "./features/diary/writer.js";
import {
  ObligationStore,
  ObligationExecutor,
  ObligationStatus,
  detectObligations,
  handleObligationConfirm,
  handleObligationReopen,
  handleEscalationRetry,
  handleEscalationDismiss,
  handleEscalationTakeover,
  OBLIGATION_CONFIRM_PREFIX,
  OBLIGATION_REOPEN_PREFIX,
  OBLIGATION_ESCALATION_RETRY_PREFIX,
  OBLIGATION_ESCALATION_DISMISS_PREFIX,
  OBLIGATION_ESCALATION_TAKEOVER_PREFIX,
} from "./features/obligations/index.js";
import { obligationKeyboard } from "./channels/telegram.js";
import {
  startReminderPoller,
  handleReminderDone,
  handleReminderSnooze,
} from "./features/reminders/poller.js";
import { serve } from "@hono/node-server";
import type { ServerType } from "@hono/node-server";
import { createHttpApp } from "./http.js";
import type { ChannelRegistryEntry } from "./http.js";

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
  const log = createLogger("nova-daemon");

  // ── Config validation (fail-fast) ─────────────────────────────────────────
  let config: Awaited<ReturnType<typeof loadConfig>>;
  try {
    config = await loadConfig();
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`\nFATAL: ${message}\n\n`);
    process.exit(1);
  }

  const version = readVersion();

  // Log source breakdown for diagnostics
  log.info(
    {
      service: "nova-daemon",
      version,
      configPath: config.configPath,
      daemonPort: config.daemonPort,
    },
    "Configuration loaded — Nova daemon starting",
  );

  // ── Fleet client initialization ───────────────────────────────────────────

  initFleetClient(config.toolRouterUrl);

  // ── Smart message router (Tier 1: keyword, Tier 2: embedding) ───────────

  const keywordRouter = new KeywordRouter();
  let embeddingRouter: EmbeddingRouter | null = null;

  try {
    embeddingRouter = await EmbeddingRouter.create();
    if (embeddingRouter) {
      log.info({ service: "nova-daemon" }, "Embedding router (Tier 2) ready");
    } else {
      log.warn({ service: "nova-daemon" }, "Embedding router disabled — Tier 2 unavailable");
    }
  } catch (err: unknown) {
    log.warn(
      { service: "nova-daemon", err: err instanceof Error ? err.message : String(err) },
      "Embedding router failed to initialize — Tier 2 disabled",
    );
  }

  const messageRouter = new MessageRouter(keywordRouter, embeddingRouter);
  log.info({ service: "nova-daemon" }, "Smart message router initialized");

  // ── Database pool ──────────────────────────────────────────────────────────

  const pool = new Pool({ connectionString: config.databaseUrl });
  const obligationStore = new ObligationStore(pool);

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

    watcher = new ProactiveWatcher(pool, telegram, config.proactiveWatcher, log, chatId, obligationStore);
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

  const briefingDeps = config.vercelGatewayKey
    ? {
        pool,
        gatewayKey: config.vercelGatewayKey,
        logger: log,
        config,
        telegram,
        telegramChatId: config.telegramChatId ?? null,
      }
    : null;

  if (briefingDeps) {
    stopBriefingScheduler = startBriefingScheduler(briefingDeps);
    log.info({ service: "nova-daemon" }, "Morning briefing scheduler started");
  } else {
    log.warn(
      { service: "nova-daemon" },
      "VERCEL_GATEWAY_KEY not set — morning briefing scheduler disabled",
    );
  }

  // ── Digest scheduler ──────────────────────────────────────────────────────

  let stopDigestScheduler: (() => void) | null = null;

  if (config.digest.enabled && telegram !== null) {
    const digestDeps = {
      pool,
      logger: log,
      telegram,
      telegramChatId: config.telegramChatId ?? null,
      config,
    };

    stopDigestScheduler = startDigestScheduler(digestDeps);
    setDigestDeps(digestDeps);

    log.info(
      {
        service: "nova-daemon",
        tier1Hours: config.digest.tier1Hours,
        realtimeIntervalMs: config.digest.realtimeIntervalMs,
        quietStart: config.digest.quietStart,
        quietEnd: config.digest.quietEnd,
      },
      "Digest scheduler started",
    );
  } else {
    log.info(
      { service: "nova-daemon", enabled: config.digest.enabled, hasTelegram: telegram !== null },
      "Digest scheduler not started",
    );
  }

  // ── Token refresh cron ─────────────────────────────────────────────────────

  const stopTokenRefresh = startTokenRefresh();

  // ── Reminder delivery poller ────────────────────────────────────────────────

  let stopReminderPoller: (() => void) | null = null;

  if (telegram !== null && config.telegramChatId) {
    stopReminderPoller = startReminderPoller({
      pool,
      logger: log,
      telegram,
      telegramChatId: config.telegramChatId,
    });
    log.info({ service: "nova-daemon" }, "Reminder delivery poller started");
  }

  // ── NovaAgent ──────────────────────────────────────────────────────────────

  const agent = await NovaAgent.create(config);
  const conversationManager = new ConversationManager(pool);

  // Wire obligation store into telegram for /obligation command
  if (telegram !== null) {
    telegram.setObligationStore(obligationStore);
  }

  log.info({ service: "nova-daemon" }, "NovaAgent ready");

  // ── Obligation executor ──────────────────────────────────────────────────────

  let obligationExecutor: ObligationExecutor | null = null;

  if (config.autonomy?.enabled && config.vercelGatewayKey && telegram !== null) {
    obligationExecutor = new ObligationExecutor(
      obligationStore,
      config.vercelGatewayKey,
      telegram,
      config.telegramChatId ?? "",
      config.autonomy,
      config.proactiveWatcher,
      config,
    );
    obligationExecutor.start();
    log.info({ service: "nova-daemon" }, "ObligationExecutor started");
  } else {
    log.info(
      {
        service: "nova-daemon",
        autonomyEnabled: config.autonomy?.enabled,
        hasGatewayKey: !!config.vercelGatewayKey,
        hasTelegram: telegram !== null,
      },
      "ObligationExecutor not started",
    );
  }

  // ── Job queue ────────────────────────────────────────────────────────────────

  const threadResolver = new ThreadResolver(pool);
  const queue = new JobQueue(config.queue);

  queue.on("started", (event) => {
    log.info(
      { service: "nova-daemon", jobId: event.job.id, chatId: event.job.chatId, queueDepth: event.queueDepth },
      "Job started",
    );
    // Send typing indicator when job starts (streaming drafts replace it once text arrives)
    if (telegram !== null) {
      void telegram.sendChatAction(event.job.chatId, "typing");
    }
  });

  queue.on("completed", (event) => {
    log.info(
      { service: "nova-daemon", jobId: event.job.id, chatId: event.job.chatId, queueDepth: event.queueDepth },
      "Job completed",
    );
  });

  queue.on("failed", (event) => {
    log.error(
      { service: "nova-daemon", jobId: event.job.id, chatId: event.job.chatId, error: event.job.error, queueDepth: event.queueDepth },
      "Job failed",
    );
    // Notify user of failure
    if (telegram !== null) {
      void telegram.sendMessage(
        event.job.chatId,
        "Sorry, something went wrong processing your message. Please try again.",
        { replyToMessageId: event.job.replyToMessageId },
      );
    }
  });

  queue.on("cancelled", (event) => {
    log.info(
      { service: "nova-daemon", jobId: event.job.id, chatId: event.job.chatId, queueDepth: event.queueDepth },
      "Job cancelled",
    );
  });

  log.info(
    { service: "nova-daemon", concurrency: config.queue.concurrency, maxQueueSize: config.queue.maxQueueSize },
    "Job queue initialized",
  );

  // ── Dream scheduler ─────────────────────────────────────────────────────────

  let dreamScheduler: DreamScheduler | null = null;

  if (config.dream.enabled) {
    dreamScheduler = new DreamScheduler(config.dream);
    dreamScheduler.start();

    // Expose the scheduler so the agent interaction hook can increment counter
    agent.setDreamScheduler(dreamScheduler);

    log.info(
      {
        service: "nova-daemon",
        cronHour: config.dream.cronHour,
        interactionThreshold: config.dream.interactionThreshold,
        sizeThresholdKb: config.dream.sizeThresholdKb,
      },
      "Dream scheduler started",
    );
  }

  // ── Channel registry ──────────────────────────────────────────────────────

  const channelRegistry: ChannelRegistryEntry[] = [
    {
      name: "Telegram",
      status: telegram !== null ? "connected" : (telegramToken ? "configured" : "unconfigured"),
      direction: "bidirectional",
    },
    // Discord and Microsoft Teams are currently managed via channels-svc,
    // not as direct daemon adapters — report as unconfigured until wired.
    {
      name: "Discord",
      status: "unconfigured",
      direction: "bidirectional",
    },
    {
      name: "Microsoft Teams",
      status: "unconfigured",
      direction: "bidirectional",
    },
  ];

  // ── HTTP server (Hono) ───────────────────────────────────────────────────

  const httpApp = createHttpApp({
    agent,
    conversationManager,
    config,
    logger: log,
    briefingDeps: briefingDeps ?? undefined,
    channelRegistry,
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

      // Route digest inline keyboard callbacks (log + acknowledge for now)
      if (data.startsWith("digest:")) {
        log.info({ service: "nova-daemon", action: data }, "Digest callback received");
        // Callbacks are out-of-scope — acknowledge and log
        return;
      }

      // Route reminder inline keyboard callbacks
      if (data.startsWith("reminder:")) {
        const callbackQueryId = String(
          (msg.metadata as { callbackQueryId?: string } | undefined)
            ?.callbackQueryId ?? "",
        );
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );

        if (data.startsWith("reminder:done:")) {
          const reminderId = data.slice("reminder:done:".length);
          void handleReminderDone(reminderId, pool, telegram!, msg.chatId, messageId, callbackQueryId);
        } else if (data.startsWith("reminder:snooze:")) {
          // Format: reminder:snooze:<duration>:<id>
          const rest = data.slice("reminder:snooze:".length);
          const lastColon = rest.lastIndexOf(":");
          const duration = rest.slice(0, lastColon);
          const reminderId = rest.slice(lastColon + 1);
          void handleReminderSnooze(reminderId, duration, pool, telegram!, msg.chatId, messageId, callbackQueryId);
        }

        return;
      }

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

      // Route escalation inline keyboard callbacks
      if (data.startsWith(OBLIGATION_ESCALATION_RETRY_PREFIX)) {
        const id = data.slice(OBLIGATION_ESCALATION_RETRY_PREFIX.length);
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );
        void handleEscalationRetry(id, obligationStore, telegram!, msg.chatId, messageId);
        return;
      }

      if (data.startsWith(OBLIGATION_ESCALATION_DISMISS_PREFIX)) {
        const id = data.slice(OBLIGATION_ESCALATION_DISMISS_PREFIX.length);
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );
        void handleEscalationDismiss(id, obligationStore, telegram!, msg.chatId, messageId);
        return;
      }

      if (data.startsWith(OBLIGATION_ESCALATION_TAKEOVER_PREFIX)) {
        const id = data.slice(OBLIGATION_ESCALATION_TAKEOVER_PREFIX.length);
        const messageId = Number(
          (msg.metadata as { originalMessageId?: number } | undefined)
            ?.originalMessageId ?? 0,
        );
        void handleEscalationTakeover(id, obligationStore, telegram!, msg.chatId, messageId);
        return;
      }

      // Route regular messages to the agent loop

      // Notify executor of activity (resets idle timer)
      if (obligationExecutor !== null) {
        obligationExecutor.notifyActivity();
      }

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

      // ── Cancel-phrase detection ───────────────────────────────────────
      const CANCEL_RE = /^(cancel|stop|never mind|nvm)$/i;

      if (CANCEL_RE.test(data.trim())) {
        const cancelled = queue.cancelByChatId(msg.chatId);
        const reply = cancelled > 0 ? "Cancelled." : "Nothing to cancel.";
        void telegram!.sendMessage(msg.chatId, reply);
        return;
      }

      // ── Ping intercept (E2E health probe) ────────────────────────────
      const PING_RE = /^ping$/i;
      if (PING_RE.test(data.trim())) {
        log.info(
          { service: "nova-daemon", chatId: msg.chatId },
          "Ping received — replying pong",
        );
        void telegram!.sendMessage(msg.chatId, "pong", {
          replyToMessageId: msg.metadata.messageId as number,
        });
        return;
      }

      void (async () => {
        try {
          // ── Smart routing: try Tier 1/2 before Agent SDK ───────────────
          const routeStart = Date.now();
          const route = await messageRouter.route(data);

          if (route.tier === 1 || route.tier === 2) {
            log.info(
              {
                service: "nova-daemon",
                chatId: msg.chatId,
                tier: route.tier,
                tool: route.tool,
                confidence: route.confidence,
              },
              "Smart route matched — bypassing Agent SDK",
            );

            let responseText: string;
            try {
              const toolResult = await fleetPost(route.port!, "/execute", {
                tool: route.tool,
                params: route.params ?? {},
              });
              responseText = formatToolResponse(toolResult);
            } catch (fleetErr: unknown) {
              log.warn(
                {
                  service: "nova-daemon",
                  tool: route.tool,
                  err: fleetErr instanceof Error ? fleetErr.message : String(fleetErr),
                },
                "Fleet tool call failed — falling through to Agent SDK",
              );
              // Fall through to agent on fleet failure
              responseText = "";
            }

            if (responseText) {
              const elapsed = Date.now() - routeStart;

              // Send response to Telegram
              try {
                await telegram!.sendMessage(msg.chatId, responseText, {
                  parseMode: "Markdown",
                  disablePreview: true,
                });
              } catch {
                // Markdown failed — send plain text
                const plain = responseText
                  .replace(/\*\*(.+?)\*\*/g, "$1")
                  .replace(/\*(.+?)\*/g, "$1")
                  .replace(/`([^`]+)`/g, "$1")
                  .replace(/```[\s\S]*?```/g, (m) =>
                    m.replace(/```\w*\n?/g, "").replace(/```/g, ""),
                  );
                await telegram!.sendMessage(msg.chatId, plain);
              }

              // Log to diary (fire-and-forget)
              void writeEntry({
                triggerType: "message",
                triggerSource: msg.senderId,
                channel: msg.channel,
                slug: msg.content.slice(0, 50),
                content: responseText,
                toolsUsed: route.tool
                  ? [buildToolCallDetail(route.tool, {}, null)]
                  : [],
                responseLatencyMs: elapsed,
                routingTier: route.tier,
                routingConfidence: route.confidence,
                model: config.agent.model,
              });

              log.info(
                {
                  service: "nova-daemon",
                  chatId: msg.chatId,
                  tier: route.tier,
                  tool: route.tool,
                  confidence: route.confidence,
                  latencyMs: elapsed,
                },
                "Smart-routed response sent",
              );

              return;
            }
          }

          // ── Tier 3: Full Agent SDK (streaming) — dispatched via job queue ──

          // Determine priority: /brief and /dream get low, regular messages get normal
          const priority = /^\/(brief|dream)\b/i.test(data) ? "low" as const : "normal" as const;

          // Resolve thread for this message before enqueueing
          const telegramMessageId = msg.metadata.messageId as number;
          const replyToMessageId = msg.replyToMessageId;
          const threadId = await threadResolver.resolve(
            msg.chatId,
            telegramMessageId,
            replyToMessageId,
          );

          let enqueueResult: EnqueueResult;
          try {
            enqueueResult = queue.enqueue({
              chatId: msg.chatId,
              threadId,
              content: data,
              priority,
              replyToMessageId: replyToMessageId ?? undefined,
              handler: async (signal: AbortSignal) => {
                const writer = new TelegramStreamWriter(telegram!, msg.chatId, telegramMessageId);

                try {
                  // Load conversation history — always channel-scoped.
                  // Thread routing controls queue ordering, not conversation context.
                  const channelKey = `telegram:${msg.chatId}`;
                  const history = await conversationManager.loadHistory(
                    channelKey,
                    config.conversationHistoryDepth,
                  );

                  let finalResponse: { text: string; toolCalls: { name: string }[]; stopReason: string } | null = null;

                  for await (const event of agent.processMessageStream(msg, history)) {
                    // Check abort before processing each event
                    if (signal.aborted) {
                      return;
                    }

                    switch (event.type) {
                      case "text_delta":
                        writer.onTextDelta(event.text);
                        break;
                      case "tool_start":
                        writer.onToolStart(event.name, event.callId);
                        break;
                      case "tool_done":
                        writer.onToolDone(event.name, event.callId, event.durationMs);
                        break;
                      case "done":
                        finalResponse = event.response;
                        // Check abort before sending final response
                        if (signal.aborted) {
                          return;
                        }
                        await writer.finalize(event.response.text);
                        break;
                    }
                  }

                  if (finalResponse && !signal.aborted) {
                    // Attach threadId so saveExchange persists it
                    msg.threadId = threadId;

                    // Save exchange fire-and-forget — never block the response path
                    void conversationManager.saveExchange(channelKey, msg, {
                      ...msg,
                      senderId: "nova",
                      senderName: "nova",
                      content: finalResponse.text,
                      text: finalResponse.text,
                    }).catch((saveErr: unknown) => {
                      log.warn(
                        { service: "nova-daemon", chatId: msg.chatId, err: saveErr },
                        "Failed to save conversation exchange",
                      );
                    });

                    log.info(
                      {
                        service: "nova-daemon",
                        chatId: msg.chatId,
                        stopReason: finalResponse.stopReason,
                        toolCalls: finalResponse.toolCalls.length,
                      },
                      "Agent response sent (streaming)",
                    );

                    // Fire-and-forget: detect obligations from the exchange
                    void (async () => {
                      try {
                        const detected = await detectObligations(
                          msg.content,
                          finalResponse!.text,
                          msg.channel,
                          config.vercelGatewayKey,
                        );

                        for (const det of detected) {
                          const record = await obligationStore.create({
                            detectedAction: det.detectedAction,
                            owner: det.owner,
                            status: ObligationStatus.Open,
                            priority: det.priority,
                            projectCode: det.projectCode,
                            sourceChannel: "telegram",
                            sourceMessage: msg.content,
                            deadline: det.deadline,
                          });

                          log.info(
                            {
                              service: "nova-daemon",
                              obligationId: record.id,
                              action: det.detectedAction,
                              owner: det.owner,
                              priority: det.priority,
                            },
                            "Obligation detected and created",
                          );

                          // Notify via Telegram for nova-owned obligations
                          if (det.owner === "nova") {
                            await telegram!.sendMessage(
                              msg.chatId,
                              `Obligation detected: <b>${det.detectedAction}</b> (P${det.priority})`,
                              {
                                parseMode: "HTML",
                                keyboard: obligationKeyboard(record.id),
                              },
                            );
                          }
                        }
                      } catch (detErr: unknown) {
                        log.warn(
                          { service: "nova-daemon", err: detErr },
                          "Obligation detection failed",
                        );
                      }
                    })();
                  }
                } catch (err: unknown) {
                  if (signal.aborted) {
                    return;
                  }
                  log.error(
                    { service: "nova-daemon", chatId: msg.chatId, err },
                    "Agent processing failed",
                  );
                  await writer.abort("Sorry, something went wrong.");
                }
              },
            });
          } catch (queueErr: unknown) {
            // Queue full
            log.warn(
              { service: "nova-daemon", chatId: msg.chatId, err: queueErr },
              "Queue full — rejecting message",
            );
            void telegram!.sendMessage(msg.chatId, "Queue full, try again in a moment.");
            return;
          }

          // Send ack if queued (not immediately started)
          if (!enqueueResult.startedImmediately) {
            let ackText: string;
            if (enqueueResult.threadState === "thread-busy") {
              ackText = enqueueResult.position <= 1
                ? "Processing your previous message. This one is next."
                : `Queued (${enqueueResult.position} ahead in this thread).`;
            } else {
              ackText = "All workers busy. You're next when one frees up.";
            }
            void telegram!.sendMessage(msg.chatId, ackText, {
              replyToMessageId: msg.metadata.messageId as number,
            });
          }
        } catch (err: unknown) {
          log.error(
            { service: "nova-daemon", chatId: msg.chatId, err },
            "Message routing failed",
          );
        }
      })();
    });
  }

  // ── Graceful shutdown ──────────────────────────────────────────────────────

  const shutdown = async (): Promise<void> => {
    log.info({ service: "nova-daemon" }, "Shutting down…");

    // Drain job queue first — cancel waiting, wait for running (up to 10s)
    const drainResult = await queue.drain(10_000);
    log.info(
      { service: "nova-daemon", cancelled: drainResult.cancelled, drained: drainResult.drained },
      "Job queue drained",
    );

    if (obligationExecutor !== null) {
      await obligationExecutor.stop();
    }

    if (stopReminderPoller !== null) {
      stopReminderPoller();
    }

    if (stopBriefingScheduler !== null) {
      stopBriefingScheduler();
    }

    if (stopDigestScheduler !== null) {
      stopDigestScheduler();
    }

    if (dreamScheduler !== null) {
      dreamScheduler.stop();
    }

    stopTokenRefresh();

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
