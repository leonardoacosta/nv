import { logger } from "../../logger.js";
import { runDream, getDreamStatus } from "./orchestrator.js";
import type { DreamSchedulerConfig } from "./types.js";

const POLL_INTERVAL_MS = 60_000; // Check every 60 seconds

/**
 * DreamScheduler — manages automatic dream triggering via cron, interaction
 * count, and memory size thresholds. Follows the ProactiveWatcher pattern
 * (start/stop lifecycle, setInterval-based polling).
 */
export class DreamScheduler {
  private _timer: ReturnType<typeof setInterval> | null = null;
  private _interactionCount = 0;
  private _running = false;
  private _lastCronDate: string | null = null;

  constructor(private readonly config: DreamSchedulerConfig) {}

  /** Start the scheduler. No-op if already running. */
  start(): void {
    if (this._timer !== null) return;

    this._timer = setInterval(() => {
      void this._tick();
    }, POLL_INTERVAL_MS);

    logger.info(
      {
        cronHour: this.config.cronHour,
        interactionThreshold: this.config.interactionThreshold,
        sizeThresholdKb: this.config.sizeThresholdKb,
        debounceHours: this.config.debounceHours,
      },
      "DreamScheduler started",
    );
  }

  /** Stop the scheduler. */
  stop(): void {
    if (this._timer !== null) {
      clearInterval(this._timer);
      this._timer = null;
    }
  }

  /**
   * Increment the interaction counter. Called from NovaAgent.processMessageStream().
   * When the counter reaches the threshold, triggers a dream and resets.
   */
  incrementInteractionCount(): void {
    this._interactionCount++;

    if (this._interactionCount >= this.config.interactionThreshold) {
      logger.info(
        { count: this._interactionCount, threshold: this.config.interactionThreshold },
        "Dream interaction threshold reached",
      );
      this._interactionCount = 0;
      void this._triggerDream("interaction");
    }
  }

  // ── Private ─────────────────────────────────────────────────────────────────

  private async _tick(): Promise<void> {
    // Check cron trigger
    const now = new Date();
    const todayStr = now.toISOString().slice(0, 10);

    if (now.getHours() === this.config.cronHour && this._lastCronDate !== todayStr) {
      this._lastCronDate = todayStr;
      logger.info({ hour: this.config.cronHour }, "Dream cron trigger firing");
      await this._triggerDream("cron");
      return;
    }

    // Check size trigger
    try {
      const status = await getDreamStatus();
      const totalKb = Math.round(status.totalSizeBytes / 1024);

      if (totalKb >= this.config.sizeThresholdKb) {
        logger.info(
          { totalKb, threshold: this.config.sizeThresholdKb },
          "Dream size threshold reached",
        );
        await this._triggerDream("size");
      }
    } catch (err) {
      logger.error({ err }, "DreamScheduler size check failed");
    }
  }

  private async _triggerDream(reason: string): Promise<void> {
    if (this._running) {
      logger.debug("Dream already running — skipping trigger");
      return;
    }

    // Debounce check: read _dream_meta for lastDreamAt
    try {
      const status = await getDreamStatus();

      if (status.lastDreamAt) {
        const lastDream = new Date(status.lastDreamAt);
        const elapsedHours = (Date.now() - lastDream.getTime()) / (1000 * 60 * 60);

        if (elapsedHours < this.config.debounceHours) {
          logger.debug(
            { reason, elapsedHours: Math.round(elapsedHours), debounceHours: this.config.debounceHours },
            "Dream debounced — skipping",
          );
          return;
        }
      }
    } catch {
      // If we can't read meta, proceed anyway
    }

    this._running = true;

    try {
      logger.info({ reason }, "Dream trigger firing");
      const result = await runDream({ topicMaxKb: this.config.topicMaxKb });
      logger.info(
        {
          reason,
          topicsProcessed: result.topicsProcessed,
          bytesBefore: result.bytesBefore,
          bytesAfter: result.bytesAfter,
          durationMs: result.durationMs,
        },
        "Scheduled dream cycle complete",
      );
    } catch (err) {
      logger.error({ err, reason }, "Scheduled dream cycle failed");
    } finally {
      this._running = false;
    }
  }
}
