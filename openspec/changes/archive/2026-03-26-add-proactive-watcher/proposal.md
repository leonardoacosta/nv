# Proposal: Add Proactive Obligation Watcher

## Change ID
`add-proactive-watcher`

## Summary

Port the proactive obligation watcher to TypeScript. A scheduled background scanner runs every 30 minutes (configurable), detects overdue, stale, and approaching-deadline obligations from Postgres, and sends Telegram reminder cards with [Mark Done] [Snooze 24h] [Dismiss] inline keyboard buttons. Quiet hours (10pm–7am, configurable) suppress notifications.

## Context

- Phase: 3 — Features | Wave: 7
- Depends on: `add-obligation-system` (Postgres-backed obligation store — provides `packages/db/src/schema/obligations.ts` and the `db` client; `TelegramAdapter` from `add-telegram-adapter`)
- Extends: `packages/daemon/src/` (TypeScript daemon from `scaffold-ts-daemon`)
- Config: `config/nv.toml` `[proactive_watcher]` section (already present)
- Rust reference: `crates/nv-daemon/src/orchestrator.rs` — existing `send_morning_briefing()`, obligation callback handling, and quiet-hours logic

## Motivation

Obligations are stored and forgotten unless something periodically checks their status. The Rust daemon has no dedicated watcher loop — it relies on the morning briefing and P0/P1 immediate notifications only. The TypeScript daemon introduces a proper watcher that:

1. Catches obligations that are overdue (past deadline), stale (no update in 48 h), or approaching deadline (within 24 h)
2. Sends targeted Telegram reminders with actionable buttons so Leo can triage from mobile without opening a separate view
3. Respects quiet hours so notifications don't wake Leo at midnight
4. Is fully configurable via `nv.toml` with safe defaults so it works out of the box with zero config

## Requirements

### Req-1: ProactiveWatcher class (`src/features/watcher/proactive.ts`)

Create a `ProactiveWatcher` class that owns the scan-and-notify lifecycle:

```typescript
export class ProactiveWatcher {
  constructor(
    private db: DbClient,
    private telegram: TelegramAdapter,
    private config: ProactiveWatcherConfig,
    private logger: Logger,
  ) {}

  start(): void;       // sets the interval; no-op if already running
  stop(): void;        // clears the interval
  async scan(): void;  // one full scan pass — exposed for testing
}
```

- `start()` calls `setInterval(() => this.scan(), intervalMs)` where `intervalMs = config.intervalMinutes * 60_000`
- `start()` also fires an immediate `scan()` on first call (so the watcher doesn't wait one full interval before the first check)
- `stop()` calls `clearInterval(this._timer)`
- The interval handle is stored privately; no external exposure

### Req-2: Quiet hours guard

Before sending any Telegram notification, check whether the current local time falls within quiet hours. If within quiet hours, log at debug level ("quiet hours — skipping notification") and return without sending.

```typescript
function isQuietHours(now: Date, config: ProactiveWatcherConfig): boolean;
```

- `config.quietStart` and `config.quietEnd` are `"HH:MM"` strings (24-hour)
- Handles the midnight wrap-around case (e.g. `22:00`–`07:00` spans midnight)
- Uses the system local timezone (no UTC conversion — same behavior as the user's clock)

### Req-3: Obligation scan queries

`scan()` runs three targeted Postgres queries via Drizzle against the `obligations` table:

| Scan Type | Condition | Label |
|-----------|-----------|-------|
| Overdue | `deadline IS NOT NULL AND deadline < NOW() AND status IN ('pending', 'in_progress')` | `overdue` |
| Stale | `updated_at < NOW() - INTERVAL '48 hours' AND status IN ('pending', 'in_progress')` | `stale` |
| Approaching deadline | `deadline IS NOT NULL AND deadline BETWEEN NOW() AND NOW() + INTERVAL '24 hours' AND status IN ('pending', 'in_progress')` | `approaching` |

Where `48 hours` uses `config.staleThresholdHours` and `24 hours` uses `config.approachingDeadlineHours`.

For each scan, apply `config.maxRemindersPerInterval` as an upper bound on notifications sent per scan pass (prevents flooding if many obligations match simultaneously). Oldest-first ordering (by `created_at ASC`).

### Req-4: Reminder card format

Each reminder is sent as a Telegram HTML-formatted message with an inline keyboard:

```
[Overdue] Deploy auth service by Friday
Status: in_progress
Overdue by: 2 days
Project: OO
```

```
[Stale] Update API docs
Status: pending
No update in: 51 hours
```

```
[Approaching] Migrate production database
Status: pending
Deadline in: 8 hours
Project: TC
```

Format rules:
- First line: `[Scan Type]` badge (uppercase) followed by `detected_action`
- `Status:` line always present
- Time context line varies by scan type: "Overdue by: N days/hours", "No update in: N hours", "Deadline in: N hours"
- `Project:` line only when `project_code` is non-null
- Use `<b>...</b>` for the badge, plain text for the rest

### Req-5: Inline keyboard

Each reminder card includes a single-row inline keyboard:

| Button | Callback Data | Effect |
|--------|--------------|--------|
| Mark Done | `watcher:done:{id}` | Set `status = 'done'`, `updated_at = NOW()` |
| Snooze 24h | `watcher:snooze:{id}` | Set `updated_at = NOW() + 24h` (resets stale clock) |
| Dismiss | `watcher:dismiss:{id}` | Set `status = 'cancelled'`, `updated_at = NOW()` |

Export a `watcherKeyboard(obligationId: string): InlineKeyboardMarkup` convenience builder that uses `buildKeyboard` from the Telegram adapter.

### Req-6: Callback handler

Export a `handleWatcherCallback(data: string, db: DbClient, telegram: TelegramAdapter, messageId: number, chatId: string): Promise<void>` function:

1. Parse the callback prefix: `watcher:done:`, `watcher:snooze:`, `watcher:dismiss:`
2. Extract obligation ID
3. Apply the appropriate Drizzle update (see Req-5)
4. Edit the original Telegram message text to show a confirmation: "Obligation marked done.", "Obligation snoozed for 24 hours.", "Obligation dismissed."
5. Remove the inline keyboard from the edited message
6. Call `telegram.answerCallbackQuery(callbackId)` to dismiss the Telegram spinner
7. Log the action at info level

The snooze operation sets `updated_at = new Date(Date.now() + 24 * 60 * 60 * 1000)`. This is a deliberate data trick: by advancing `updated_at` into the future, the next scan will not see this obligation as stale for 24 h without changing its status.

### Req-7: Config type (`src/features/watcher/types.ts`)

```typescript
export interface ProactiveWatcherConfig {
  enabled: boolean;               // default: true
  intervalMinutes: number;        // default: 30
  staleThresholdHours: number;    // default: 48
  approachingDeadlineHours: number; // default: 24
  maxRemindersPerInterval: number; // default: 1
  quietStart: string;             // default: "22:00"
  quietEnd: string;               // default: "07:00"
}
```

Map from `nv.toml` `[proactive_watcher]` section (already present in `config/nv.toml`). Provide `defaultProactiveWatcherConfig` as a const fallback used when the section is absent.

### Req-8: Config loader integration

Extend `packages/daemon/src/config.ts`:

- Add `proactiveWatcher: ProactiveWatcherConfig` to the `Config` type
- In `loadConfig()`, read `config.proactive_watcher` from the TOML (snake_case keys from file) and map to camelCase `ProactiveWatcherConfig`; merge with `defaultProactiveWatcherConfig` for missing keys

### Req-9: Daemon wiring (`src/index.ts`)

In `packages/daemon/src/index.ts`:

- After the agent and Telegram adapter are initialized, instantiate `ProactiveWatcher` with `db`, `telegram`, `config.proactiveWatcher`, and `logger`
- If `config.proactiveWatcher.enabled`, call `watcher.start()`
- On process SIGTERM/SIGINT, call `watcher.stop()` before exiting
- Log at info level: "Proactive watcher started (interval: 30min, quiet: 22:00–07:00)"

Route incoming callback queries with prefix `watcher:` to `handleWatcherCallback`.

## Scope

- **IN**: `ProactiveWatcher` class, three scan queries (overdue/stale/approaching), reminder card formatter, `watcherKeyboard`, `handleWatcherCallback`, `ProactiveWatcherConfig` type, config loader extension, daemon wiring
- **OUT**: Watcher dashboard page (separate concern), per-obligation notification deduplication (first pass: send on every scan), per-user quiet hours override, reminder history tracking, obligation auto-execution (separate `add-autonomous-obligation-execution` spec)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/watcher/proactive.ts` | New: `ProactiveWatcher` class |
| `packages/daemon/src/features/watcher/types.ts` | New: `ProactiveWatcherConfig`, `defaultProactiveWatcherConfig` |
| `packages/daemon/src/features/watcher/callbacks.ts` | New: `handleWatcherCallback`, `watcherKeyboard` |
| `packages/daemon/src/features/watcher/index.ts` | New: barrel re-export |
| `packages/daemon/src/config.ts` | Modified: add `proactiveWatcher` field to `Config`, load from TOML |
| `packages/daemon/src/index.ts` | Modified: instantiate + start watcher, wire callback routing, SIGTERM stop |

No changes to `packages/db/` schema (obligations table owned by `add-obligation-system`). No changes to Rust codebase.

## Risks

| Risk | Mitigation |
|------|-----------|
| Many matching obligations flood Telegram | `maxRemindersPerInterval` cap (default 1) limits sends per scan pass |
| Quiet hours timezone mismatch | Use `Date` local time (system TZ); document that server TZ must match user TZ in README |
| Snooze via `updated_at` future-dating is a data trick that could confuse queries | Document explicitly in code comment; `ObligationStore` queries use `updated_at < NOW()` so future dates are naturally excluded |
| Callback query ID expires (60s limit) | `answerCallbackQuery` called first in handler before any DB operations |
| `add-obligation-system` not yet applied | Hard gate in tasks.md — obligations table must exist before watcher starts |
| Watcher fires during initial startup before Telegram adapter is ready | `start()` deferred until after Telegram adapter init in `index.ts` wiring order |
