import type { BriefingDeps } from "./synthesizer.js";
import { runMorningBriefing } from "./runner.js";

// ─── startBriefingScheduler ───────────────────────────────────────────────────

const POLL_INTERVAL_MS = 60_000; // 60 seconds
const BRIEFING_HOUR = 7;

/**
 * Starts a polling scheduler that fires the morning briefing at 07:00 local time.
 *
 * - Polls every 60 seconds.
 * - Checks the `briefings` table for today's date to prevent double-fire.
 *   This is resilient across daemon restarts — if the daemon restarts at 7:30am
 *   and a briefing was already generated at 7:01am, it will NOT fire again.
 * - On first startup during the briefing hour, if no briefing exists for today,
 *   it fires immediately (catches missed briefings from restarts).
 * - Fires `runMorningBriefing` as fire-and-forget — errors are logged but never thrown.
 * - After briefingHour + 1, checks for missed briefing and sends Telegram alert
 *   if no briefing exists for today. Alerts at most once per day.
 *
 * Returns a cleanup function that clears the interval.
 */
export function startBriefingScheduler(deps: BriefingDeps): () => void {
  // In-memory guard for the current tick only — the DB is the source of truth
  let firingInProgress = false;
  // In-memory guard: track which date we've already sent a missed-briefing alert
  let missedAlertSentDate: string | null = null;

  async function checkAndFire(): Promise<void> {
    const now = new Date();
    const currentHour = now.getHours();
    const todayStr = now.toISOString().slice(0, 10);

    // ── Missed-briefing detection ────────────────────────────────────────────
    // Check after briefingHour + 1 (e.g., 08:00 if briefingHour is 7)
    if (
      currentHour >= BRIEFING_HOUR + 1 &&
      missedAlertSentDate !== todayStr
    ) {
      try {
        const missedCheck = await deps.pool.query<{ count: string }>(
          `SELECT count(*) as count FROM briefings
           WHERE generated_at::date = CURRENT_DATE`,
        );
        const briefingCount = parseInt(missedCheck.rows[0]?.count ?? "0", 10);

        if (briefingCount === 0) {
          // Mark as alerted for today before sending to prevent duplicates on rapid polls
          missedAlertSentDate = todayStr;

          deps.logger.warn(
            { date: todayStr, briefingHour: BRIEFING_HOUR },
            "Missed-briefing alert: no briefing generated today",
          );

          if (deps.telegram && deps.telegramChatId) {
            void deps.telegram
              .sendMessage(
                deps.telegramChatId,
                `No morning briefing was generated today. The daemon may have been offline at ${BRIEFING_HOUR}:00. Use the dashboard 'Generate Now' button to create one.`,
                { parseMode: "Markdown", disablePreview: true },
              )
              .catch((err: unknown) => {
                deps.logger.warn({ err }, "Missed-briefing: failed to send Telegram alert");
              });
          }
        } else {
          // Briefing exists — mark today as handled to stop checking until tomorrow
          missedAlertSentDate = todayStr;
        }
      } catch (err: unknown) {
        deps.logger.error({ err }, "Missed-briefing check: DB query failed");
      }
    }

    // ── Morning briefing fire ────────────────────────────────────────────────
    if (currentHour !== BRIEFING_HOUR) {
      return;
    }

    if (firingInProgress) {
      return;
    }

    // Check the database for today's briefing (resilient across restarts)
    try {
      const result = await deps.pool.query<{ count: string }>(
        `SELECT count(*) as count FROM briefings
         WHERE generated_at::date = CURRENT_DATE`,
      );

      const count = parseInt(result.rows[0]?.count ?? "0", 10);

      if (count > 0) {
        return; // Already generated today — skip
      }
    } catch (err: unknown) {
      deps.logger.error({ err }, "Briefing scheduler: DB check failed — skipping this tick");
      return;
    }

    firingInProgress = true;

    deps.logger.info(
      { date: todayStr, hour: BRIEFING_HOUR },
      "Morning briefing scheduler firing",
    );

    void runMorningBriefing(deps)
      .catch((err: unknown) => {
        deps.logger.error({ err }, "Morning briefing run failed");
      })
      .finally(() => {
        firingInProgress = false;
      });
  }

  const interval = setInterval(() => {
    void checkAndFire();
  }, POLL_INTERVAL_MS);

  // Fire immediately on startup to catch missed briefings
  void checkAndFire();

  return () => {
    clearInterval(interval);
  };
}
