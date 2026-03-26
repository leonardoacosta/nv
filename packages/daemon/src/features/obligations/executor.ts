import { query } from "@anthropic-ai/claude-agent-sdk";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";
import type TelegramBot from "node-telegram-bot-api";
import { ObligationStatus, type ObligationRecord } from "./types.js";
import type { ObligationStore } from "./store.js";
import { createLogger } from "../../logger.js";
import { OBLIGATION_CONFIRM_PREFIX, OBLIGATION_REOPEN_PREFIX } from "./callbacks.js";

const log = createLogger("obligation-executor");

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ExecutorConfig {
  enabled: boolean;
  timeoutMs: number;
  cooldownHours: number;
  idleDebounceMs: number;
  pollIntervalMs: number;
}

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

// ─── Timeout helper ───────────────────────────────────────────────────────────

function createTimeout(ms: number): Promise<never> {
  return new Promise<never>((_, reject) => {
    setTimeout(() => {
      reject(new Error(`Execution timed out after ${ms}ms`));
    }, ms);
  });
}

// ─── Agent SDK query wrapper ──────────────────────────────────────────────────

async function runAgentQuery(
  prompt: string,
  gatewayKey: string,
  timeoutMs: number,
): Promise<string> {
  const queryStream = query({
    prompt,
    options: {
      allowedTools: ["Read", "Write", "Bash", "Glob", "Grep", "WebSearch", "WebFetch"],
      permissionMode: "bypassPermissions",
      allowDangerouslySkipPermissions: true,
      maxTurns: 30,
      env: {
        ANTHROPIC_BASE_URL: "https://ai-gateway.vercel.sh",
        ANTHROPIC_CUSTOM_HEADERS: `x-ai-gateway-api-key: Bearer ${gatewayKey}`,
      },
    },
  });

  let resultText = "";

  const queryPromise = (async () => {
    for await (const message of queryStream as AsyncIterable<SDKMessage>) {
      if (message.type === "result") {
        if (message.subtype === "success") {
          resultText = message.result;
        } else {
          throw new Error(`Agent query failed: ${message.subtype}`);
        }
      }
    }
    return resultText;
  })();

  return Promise.race([queryPromise, createTimeout(timeoutMs)]);
}

// ─── ObligationExecutor ───────────────────────────────────────────────────────

export class ObligationExecutor {
  private lastActivityAt: number = Date.now();
  private isExecuting: boolean = false;
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  // Draining: resolves when isExecuting transitions to false
  private drainingResolvers: Array<() => void> = [];

  constructor(
    private readonly store: ObligationStore,
    private readonly gatewayKey: string,
    private readonly telegram: TelegramNotifier,
    private readonly telegramChatId: number | string,
    private readonly config: ExecutorConfig,
  ) {}

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
    const idleMs = Date.now() - this.lastActivityAt;
    if (idleMs > this.config.idleDebounceMs && !this.isExecuting) {
      void this.tryExecuteNext();
    }
  }

  private async tryExecuteNext(): Promise<void> {
    const candidates = await this.store.listReadyForExecution(
      this.config.cooldownHours,
    );

    if (candidates.length === 0) {
      return;
    }

    const obligation = candidates[0]!;

    this.isExecuting = true;
    log.info(
      { id: obligation.id, action: obligation.detectedAction },
      "Executing obligation",
    );

    try {
      await this.store.updateStatus(obligation.id, ObligationStatus.InProgress);
      await this.store.updateLastAttemptAt(obligation.id, new Date());

      const prompt = buildExecutionPrompt(obligation);
      const summary = await runAgentQuery(
        prompt,
        this.gatewayKey,
        this.config.timeoutMs,
      );

      if (!summary || summary.trim().length === 0) {
        await this.handleFailure(obligation, new Error("Agent returned empty response"));
      } else {
        await this.handleSuccess(obligation, summary);
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
      // Keep status as in_progress — cooldown prevents immediate retry

      const messageText = `Failed to complete: ${obligation.detectedAction} — ${error.message}`;
      await this.telegram.sendMessage(this.telegramChatId, messageText);

      log.warn(
        { id: obligation.id, error: error.message },
        "Obligation execution failed",
      );
    } catch (innerErr: unknown) {
      log.error({ innerErr, id: obligation.id }, "Error in failure handler");
    }
  }
}
