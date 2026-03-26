// ─── ProactiveWatcherConfig ───────────────────────────────────────────────────

export interface ProactiveWatcherConfig {
  /** Whether the watcher is active. Default: true */
  enabled: boolean;
  /** How often to scan for obligation reminders (minutes). Default: 30 */
  intervalMinutes: number;
  /** Obligations with no update for this many hours are "stale". Default: 48 */
  staleThresholdHours: number;
  /** Obligations with a deadline within this many hours are "approaching". Default: 24 */
  approachingDeadlineHours: number;
  /** Maximum notifications sent per scan interval (prevents flooding). Default: 1 */
  maxRemindersPerInterval: number;
  /** Quiet hours start in HH:MM 24-hour format. Default: "22:00" */
  quietStart: string;
  /** Quiet hours end in HH:MM 24-hour format. Default: "07:00" */
  quietEnd: string;
}

export const defaultProactiveWatcherConfig: ProactiveWatcherConfig = {
  enabled: true,
  intervalMinutes: 30,
  staleThresholdHours: 48,
  approachingDeadlineHours: 24,
  maxRemindersPerInterval: 1,
  quietStart: "22:00",
  quietEnd: "07:00",
};
