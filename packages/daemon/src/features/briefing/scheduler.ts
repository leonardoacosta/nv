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
 *
 * Returns a cleanup function that clears the interval.
 */
export function startBriefingScheduler(deps: BriefingDeps): () => void {
  // In-memory guard for the current tick only — the DB is the source of truth
  let firingInProgress = false;

  async function checkAndFire(): Promise<void> {
    const now = new Date();

    if (now.getHours() !== BRIEFING_HOUR) {
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

    const todayStr = now.toISOString().slice(0, 10);
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
