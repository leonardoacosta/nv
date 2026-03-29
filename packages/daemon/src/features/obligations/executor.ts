import type TelegramBot from "node-telegram-bot-api";
import { ObligationStatus, type ObligationRecord } from "./types.js";
import type { ObligationStore } from "./store.js";
import { createLogger } from "../../logger.js";
import {
  OBLIGATION_CONFIRM_PREFIX,
  OBLIGATION_REOPEN_PREFIX,
  OBLIGATION_ESCALATION_RETRY_PREFIX,
  OBLIGATION_ESCALATION_DISMISS_PREFIX,
  OBLIGATION_ESCALATION_TAKEOVER_PREFIX,
} from "./callbacks.js";
import type { AutonomyConfig, Config } from "../../config.js";
import type { ProactiveWatcherConfig } from "../../features/watcher/types.js";
import { isQuietHours } from "../../lib/quiet-hours.js";
import { buildMcpServers, buildAllowedTools, type McpStdioServerConfig } from "../../brain/mcp-config.js";
import { createAgentQuery } from "../../brain/query-factory.js";

const log = createLogger("obligation-executor");

/** Built-in Agent SDK tools available to the executor. */
const BUILTIN_TOOLS = ["Read", "Write", "Bash", "Glob", "Grep", "WebSearch", "WebFetch"];

// ─── Cost constants ──────────────────────────────────────────────────────────

/** Sonnet pricing per million tokens */
const SONNET_INPUT_PER_M = 3.0;
const SONNET_OUTPUT_PER_M = 15.0;

/** Haiku pricing per million tokens */
const HAIKU_INPUT_PER_M = 0.80;
const HAIKU_OUTPUT_PER_M = 4.0;

// ─── Types ────────────────────────────────────────────────────────────────────

export type ExecutorConfig = AutonomyConfig;

export interface TelegramNotifier {
  sendMessage(
    chatId: number | string,
    text: string,
    options?: {
      parseMode?: "HTML" | "Markdown" | "MarkdownV2";
      keyboard?: TelegramBot.InlineKeyboardMarkup;
    },
  ): Promise<unknown>;
}

// ─── Prompt builder ───────────────────────────────────────────────────────────

export function buildExecutionPrompt(obligation: ObligationRecord): string {
  const project = obligation.projectCode ?? "general";
  const source = obligation.sourceMessage ?? "no source message";

  return `You are Nova. You have an obligation to complete:

**Action**: ${obligation.detectedAction}
**Priority**: P${obligation.priority}
**Project**: ${project}
**Source**: ${obligation.sourceChannel} — "${source}"

Use your available tools to fulfill this obligation completely. When finished, provide a concise
summary (3–5 sentences) of what you accomplished and any relevant findings.`;
}

// ─── Model routing ───────────────────────────────────────────────────────────

/**
 * P0-P1 (urgent/critical): Sonnet for best reasoning.
 * P2+ (normal/low): Haiku for cost savings.
 */
export function selectModel(priority: number): string {
  return priority <= 1 ? "claude-sonnet-4-6" : "claude-haiku-3-5";
}

/**
 * Estimates cost in USD for a given token usage and model.
 */
export function estimateCost(
  inputTokens: number,
  outputTokens: number,
  model: string,
): number {
  const isHaiku = model.includes("haiku");
  const inputRate = isHaiku ? HAIKU_INPUT_PER_M : SONNET_INPUT_PER_M;
  const outputRate = isHaiku ? HAIKU_OUTPUT_PER_M : SONNET_OUTPUT_PER_M;
  return (inputTokens / 1_000_000) * inputRate + (outputTokens / 1_000_000) * outputRate;
}

// ─── ObligationExecutor ───────────────────────────────────────────────────────

export class ObligationExecutor {
  private lastActivityAt: number = Date.now();
  private isExecuting: boolean = false;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private readonly mcpServers: Record<string, McpStdioServerConfig>;
  private readonly allowedTools: string[];

  // Budget gate: in-memory daily spend tracker
  private dailySpendUsd: number = 0;
  private dailyResetDate: string = new Date().toISOString().slice(0, 10);

  // Draining: resolves when isExecuting transitions to false
  private drainingResolvers: Array<() => void> = [];

  constructor(
    private readonly store: ObligationStore,
    private readonly gatewayKey: string,
    private readonly telegram: TelegramNotifier,
    private readonly telegramChatId: number | string,
    private readonly config: ExecutorConfig,
    private readonly watcherConfig: ProactiveWatcherConfig,
    appConfig?: Config,
  ) {
    this.mcpServers = appConfig ? buildMcpServers(appConfig) : {};
    this.allowedTools = buildAllowedTools(this.mcpServers, BUILTIN_TOOLS);
  }

  /**
   * Resets the idle debounce timer. Call on every inbound/outbound message.
   */
  notifyActivity(): void {
    this.lastActivityAt = Date.now();
  }

  /**
   * Starts the poll loop. Non-blocking.
   */
  start(): void {
    if (!this.config.enabled) {
      log.info("ObligationExecutor disabled — skipping start");
      return;
    }

    this.pollTimer = setInterval(() => {
      void this.tick();
    }, this.config.pollIntervalMs);

    log.info(
      {
        pollIntervalMs: this.config.pollIntervalMs,
        idleDebounceMs: this.config.idleDebounceMs,
        dailyBudgetUsd: this.config.dailyBudgetUsd,
        maxAttempts: this.config.maxAttempts,
      },
      "ObligationExecutor started",
    );
  }

  /**
   * Gracefully shuts down. If an execution is in-flight, waits for it to complete.
   */
  async stop(): Promise<void> {
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }

    if (this.isExecuting) {
      await new Promise<void>((resolve) => {
        this.drainingResolvers.push(resolve);
      });
    }

    log.info("ObligationExecutor stopped");
  }

  // ── Private ────────────────────────────────────────────────────────────────

  private tick(): void {
    // Quiet hours check
    if (isQuietHours(new Date(), this.watcherConfig.quietStart, this.watcherConfig.quietEnd)) {
      return;
    }

    const idleMs = Date.now() - this.lastActivityAt;
    if (idleMs > this.config.idleDebounceMs && !this.isExecuting) {
      void this.tryExecuteNext();
    }
  }

  /**
   * Resets the daily spend counter if the UTC date has rolled over.
   */
  private resetDailySpendIfNeeded(): void {
    const today = new Date().toISOString().slice(0, 10);
    if (today !== this.dailyResetDate) {
      this.dailySpendUsd = 0;
      this.dailyResetDate = today;
      log.info({ date: today }, "Daily budget counter reset");
    }
  }

  private async tryExecuteNext(): Promise<void> {
    // Budget gate
    this.resetDailySpendIfNeeded();
    if (this.dailySpendUsd >= this.config.dailyBudgetUsd) {
      log.info(
        { dailySpendUsd: this.dailySpendUsd, budgetUsd: this.config.dailyBudgetUsd },
        "Daily budget exhausted — skipping obligation execution",
      );
      return;
    }

    const candidates = await this.store.listReadyForExecution(
      this.config.cooldownHours,
    );

    if (candidates.length === 0) {
      return;
    }

    const obligation = candidates[0]!;

    this.isExecuting = true;
    const model = selectModel(obligation.priority);
    log.info(
      { id: obligation.id, action: obligation.detectedAction, model, priority: obligation.priority },
      "Executing obligation",
    );

    try {
      await this.store.updateStatus(obligation.id, ObligationStatus.InProgress);
      await this.store.updateLastAttemptAt(obligation.id, new Date());

      const prompt = buildExecutionPrompt(obligation);
      const result = await createAgentQuery({
        prompt,
        gatewayKey: this.gatewayKey,
        timeoutMs: this.config.timeoutMs,
        mcpServers: this.mcpServers,
        allowedTools: this.allowedTools,
        model,
        maxTurns: 30,
      });

      // Track spend
      const cost = estimateCost(result.inputTokens, result.outputTokens, model);
      this.dailySpendUsd += cost;
      log.info(
        {
          id: obligation.id,
          inputTokens: result.inputTokens,
          outputTokens: result.outputTokens,
          costUsd: cost.toFixed(4),
          dailySpendUsd: this.dailySpendUsd.toFixed(4),
        },
        "Obligation execution cost tracked",
      );

      if (!result.text || result.text.trim().length === 0) {
        await this.handleFailure(obligation, new Error("Agent returned empty response"));
      } else {
        await this.handleSuccess(obligation, result.text);
      }
    } catch (err: unknown) {
      const error = err instanceof Error ? err : new Error(String(err));
      await this.handleFailure(obligation, error);
    } finally {
      this.isExecuting = false;
      // Notify any drain waiters
      for (const resolve of this.drainingResolvers) {
        resolve();
      }
      this.drainingResolvers = [];
    }
  }

  private async handleSuccess(
    obligation: ObligationRecord,
    summary: string,
  ): Promise<void> {
    const timestamp = new Date().toISOString();
    const truncated = summary.length > 500 ? summary.slice(0, 497) + "..." : summary;

    try {
      await this.store.appendNote(
        obligation.id,
        `[Auto-executed ${timestamp}] ${truncated}`,
      );
      await this.store.updateStatus(obligation.id, ObligationStatus.ProposedDone);

      const keyboard: TelegramBot.InlineKeyboardMarkup = {
        inline_keyboard: [
          [
            {
              text: "Confirm Done",
              callback_data: `${OBLIGATION_CONFIRM_PREFIX}${obligation.id}`,
            },
            {
              text: "Reopen",
              callback_data: `${OBLIGATION_REOPEN_PREFIX}${obligation.id}`,
            },
          ],
        ],
      };

      const messageText = `Nova completed: <b>${obligation.detectedAction}</b>\n\n${truncated}`;
      await this.telegram.sendMessage(this.telegramChatId, messageText, {
        parseMode: "HTML",
        keyboard,
      });

      log.info({ id: obligation.id }, "Obligation execution succeeded");
    } catch (err: unknown) {
      log.error({ err, id: obligation.id }, "Error in success handler");
    }
  }

  private async handleFailure(
    obligation: ObligationRecord,
    error: Error,
  ): Promise<void> {
    const timestamp = new Date().toISOString();

    try {
      await this.store.appendNote(
        obligation.id,
        `[Attempt failed ${timestamp}] ${error.message}`,
      );

      // Escalation: increment attempt count and check threshold
      const attemptCount = await this.store.incrementAttemptCount(obligation.id);

      if (attemptCount >= this.config.maxAttempts) {
        // Escalate
        await this.store.updateStatus(obligation.id, ObligationStatus.Escalated);

        const keyboard: TelegramBot.InlineKeyboardMarkup = {
          inline_keyboard: [
            [
              {
                text: "Retry",
                callback_data: `${OBLIGATION_ESCALATION_RETRY_PREFIX}${obligation.id}`,
              },
              {
                text: "Dismiss",
                callback_data: `${OBLIGATION_ESCALATION_DISMISS_PREFIX}${obligation.id}`,
              },
              {
                text: "Take Over",
                callback_data: `${OBLIGATION_ESCALATION_TAKEOVER_PREFIX}${obligation.id}`,
              },
            ],
          ],
        };

        const messageText =
          `Escalated after ${attemptCount} attempts: <b>${obligation.detectedAction}</b>\n` +
          `Last error: ${error.message}`;
        await this.telegram.sendMessage(this.telegramChatId, messageText, {
          parseMode: "HTML",
          keyboard,
        });

        log.warn(
          { id: obligation.id, attemptCount },
          "Obligation escalated after max attempts",
        );
      } else {
        // Normal failure — keep status as in_progress, cooldown prevents immediate retry
        const messageText = `Failed to complete: ${obligation.detectedAction} — ${error.message} (attempt ${attemptCount}/${this.config.maxAttempts})`;
        await this.telegram.sendMessage(this.telegramChatId, messageText);

        log.warn(
          { id: obligation.id, attemptCount, error: error.message },
          "Obligation execution failed",
        );
      }
    } catch (innerErr: unknown) {
      log.error({ innerErr, id: obligation.id }, "Error in failure handler");
    }
  }
}
