import type { BriefingDeps } from "./synthesizer.js";
import { runMorningBriefing } from "./runner.js";

// ─── startBriefingScheduler ───────────────────────────────────────────────────

const POLL_INTERVAL_MS = 60_000; // 60 seconds
const BRIEFING_HOUR = 7;

/**
 * Starts a polling scheduler that fires the morning briefing at 07:00 local time.
 *
 * - Polls every 60 seconds.
 * - Tracks `lastBriefingDate` (YYYY-MM-DD) to prevent double-fire on the same day.
 * - Fires `runMorningBriefing` as fire-and-forget — errors are logged but never thrown.
 *
 * Returns a cleanup function that clears the interval.
 */
export function startBriefingScheduler(deps: BriefingDeps): () => void {
  let lastBriefingDate: string | null = null;

  const interval = setInterval(() => {
    const now = new Date();
    const todayStr = now.toISOString().slice(0, 10); // YYYY-MM-DD

    if (now.getHours() !== BRIEFING_HOUR) {
      return;
    }

    if (lastBriefingDate === todayStr) {
      return; // Already fired today
    }

    lastBriefingDate = todayStr;

    deps.logger.info(
      { date: todayStr, hour: BRIEFING_HOUR },
      "Morning briefing scheduler firing",
    );

    void runMorningBriefing(deps).catch((err: unknown) => {
      deps.logger.error({ err }, "Morning briefing run failed");
    });
  }, POLL_INTERVAL_MS);

  return () => {
    clearInterval(interval);
  };
}
