# Implementation Tasks

<!-- beads:epic:nv-qxs5 -->

## Prerequisite Gate

- [x] [0.1] Verify `add-obligation-system` is applied — obligations table exists in Postgres and `packages/db/src/schema/obligations.ts` is importable [owner:api-engineer]
- [x] [0.2] Verify `add-telegram-adapter` is applied — `TelegramAdapter`, `buildKeyboard`, and `KeyboardButton` are importable from `src/channels/telegram.ts` [owner:api-engineer]

## Foundation Batch

- [x] [1.1] Create `packages/daemon/src/features/watcher/types.ts` — export `ProactiveWatcherConfig` interface (fields: `enabled`, `intervalMinutes`, `staleThresholdHours`, `approachingDeadlineHours`, `maxRemindersPerInterval`, `quietStart`, `quietEnd`) and `defaultProactiveWatcherConfig` const with defaults: `enabled=true, intervalMinutes=30, staleThresholdHours=48, approachingDeadlineHours=24, maxRemindersPerInterval=1, quietStart="22:00", quietEnd="07:00"` [owner:api-engineer]
- [x] [1.2] Extend `packages/daemon/src/config.ts` — add `proactiveWatcher: ProactiveWatcherConfig` to `Config` type; in `loadConfig()` read `config.proactive_watcher` from TOML (snake_case), map to camelCase fields, merge with `defaultProactiveWatcherConfig` for any missing keys [owner:api-engineer]

## Feature Batch

- [x] [2.1] Create `packages/daemon/src/features/watcher/proactive.ts` — `ProactiveWatcher` class with `constructor(db, telegram, config, logger)`, `start()` (setInterval + immediate first scan), `stop()` (clearInterval), `async scan()` (full pass); export class and `isQuietHours(now, config)` helper [owner:api-engineer]
- [x] [2.2] Implement `isQuietHours(now: Date, config: ProactiveWatcherConfig): boolean` — parse `config.quietStart` and `config.quietEnd` as HH:MM 24-hour strings, compute whether `now` falls in the quiet window; handle midnight wrap-around (e.g. 22:00–07:00 spans midnight) [owner:api-engineer]
- [x] [2.3] Implement `scan()` — three Drizzle queries against `obligations` table: (a) overdue: `deadline < now AND status IN ('pending','in_progress')`, (b) stale: `updated_at < now - staleThresholdHours AND status IN ('pending','in_progress')`, (c) approaching: `deadline BETWEEN now AND now + approachingDeadlineHours AND status IN ('pending','in_progress')`; collect results, apply `maxRemindersPerInterval` cap (oldest-first), call quiet hours guard before sending each [owner:api-engineer]
- [x] [2.4] Implement reminder card formatter — `formatReminderCard(obligation, scanType: 'overdue' | 'stale' | 'approaching'): string` — HTML-formatted message: bold badge line, status, time-context line (overdue by/no update in/deadline in with humanized duration), optional project code line [owner:api-engineer]
- [x] [2.5] Create `packages/daemon/src/features/watcher/callbacks.ts` — export `watcherKeyboard(obligationId: string): InlineKeyboardMarkup` (3 buttons: "Mark Done" → `watcher:done:{id}`, "Snooze 24h" → `watcher:snooze:{id}`, "Dismiss" → `watcher:dismiss:{id}`) using `buildKeyboard` from TelegramAdapter [owner:api-engineer]
- [x] [2.6] Implement `handleWatcherCallback(data, db, telegram, messageId, chatId, callbackQueryId): Promise<void>` — parse prefix, extract obligation ID, apply Drizzle update (done → status='done'; snooze → updated_at=now+24h; dismiss → status='cancelled'), edit original Telegram message with confirmation text + no keyboard, call `answerCallbackQuery` first [owner:api-engineer]
- [x] [2.7] Create `packages/daemon/src/features/watcher/index.ts` — barrel export: `ProactiveWatcher`, `handleWatcherCallback`, `watcherKeyboard`, `isQuietHours`, `ProactiveWatcherConfig`, `defaultProactiveWatcherConfig` [owner:api-engineer]
- [x] [2.8] Wire watcher into `packages/daemon/src/index.ts` — import `ProactiveWatcher` from watcher feature, instantiate after Telegram adapter init, call `watcher.start()` if `config.proactiveWatcher.enabled`; register SIGTERM/SIGINT handler calling `watcher.stop()`; route callback queries with `watcher:` prefix to `handleWatcherCallback`; log startup message [owner:api-engineer]

## Verify

- [x] [3.1] Unit test: `isQuietHours` — returns true for time inside 22:00–07:00 window (midnight wrap), returns false for time outside window, returns false when `quietStart === quietEnd` [owner:api-engineer]
- [x] [3.2] Unit test: `formatReminderCard` — overdue obligation with project code produces expected HTML; stale obligation without project code omits project line; approaching obligation shows correct badge [owner:api-engineer]
- [x] [3.3] Unit test: `watcherKeyboard` — returns keyboard with 3 buttons, each with correct `callback_data` prefix and obligation ID [owner:api-engineer]
- [x] [3.4] Unit test: `scan()` with mocked DB — `maxRemindersPerInterval=1` sends at most 1 notification even when 3 obligations match; quiet hours suppresses all sends [owner:api-engineer]
- [x] [3.5] Unit test: `handleWatcherCallback` — `watcher:done:{id}` sets status done and edits message; `watcher:snooze:{id}` advances `updated_at` by 24h and edits message; `watcher:dismiss:{id}` sets status cancelled and edits message; unknown prefix is a no-op [owner:api-engineer]
- [x] [3.6] `npm run typecheck` passes in `packages/daemon/` — zero TypeScript errors [owner:api-engineer]
- [x] [3.7] `npm run build` produces `dist/index.js` without errors [owner:api-engineer]
