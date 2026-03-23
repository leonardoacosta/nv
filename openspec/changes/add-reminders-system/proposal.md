# Proposal: Reminders System

## Change ID
`add-reminders-system`

## Summary

User-facing reminder/timer system that lets the user say "Remind me to check the CT deploy in 2
hours" and get a Telegram message when the time is due. Claude calls `set_reminder`, `list_reminders`,
and `cancel_reminder` tools. Reminders are persisted in SQLite and survive daemon restarts. A
background tokio task polls every 30s for due reminders and fires them to the originating channel.

## Context
- Extends: `crates/nv-daemon/src/messages.rs` (SQLite patterns), `crates/nv-daemon/src/tools.rs` (tool registration + dispatch), `crates/nv-daemon/src/main.rs` (background task spawning, SharedDeps), `crates/nv-daemon/src/scheduler.rs` (cron task patterns)
- Related: Existing `scheduler.rs` spawns a periodic tokio task for digests — the reminder scheduler follows the same pattern but polls SQLite instead of a fixed interval. `MessageStore` demonstrates the SQLite init + query patterns (rusqlite, `Connection::open`, `CREATE TABLE IF NOT EXISTS`).
- Depends on: nothing — standalone feature

## Motivation

Nova's scheduler can fire periodic digests, but the user has no way to set personal timers or
reminders. "Remind me to X in Y" is one of the most natural assistant interactions, and without it
the user must rely on external timer apps. Since Nova already has SQLite persistence (messages.db)
and background task infrastructure (scheduler.rs), adding reminders requires minimal new
infrastructure — just a new table, three tools, and a polling task.

## Requirements

### Req-1: SQLite Reminders Table

New table in the existing `~/.nv/messages.db` database (reuse the `MessageStore` connection):

```sql
CREATE TABLE IF NOT EXISTS reminders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message TEXT NOT NULL,
    due_at TEXT NOT NULL,          -- ISO 8601 UTC datetime
    channel TEXT NOT NULL,         -- 'telegram' | 'discord' | 'teams' | 'imessage' | 'email'
    created_at TEXT NOT NULL,      -- ISO 8601 UTC
    delivered_at TEXT,             -- NULL until fired, then ISO 8601 UTC
    cancelled INTEGER DEFAULT 0   -- 0 = active, 1 = cancelled
);

CREATE INDEX IF NOT EXISTS idx_reminders_due_at ON reminders(due_at);
CREATE INDEX IF NOT EXISTS idx_reminders_active ON reminders(cancelled, delivered_at);
```

Methods on `MessageStore` (or a new `ReminderStore` struct sharing the same db path):
- `create_reminder(message, due_at, channel) -> Result<i64>` — returns the new reminder ID
- `list_active_reminders() -> Result<Vec<Reminder>>` — returns reminders where `cancelled = 0 AND delivered_at IS NULL`, ordered by `due_at ASC`
- `cancel_reminder(id) -> Result<bool>` — sets `cancelled = 1`, returns whether the row existed
- `get_due_reminders() -> Result<Vec<Reminder>>` — returns reminders where `due_at <= now AND cancelled = 0 AND delivered_at IS NULL`
- `mark_delivered(id) -> Result<()>` — sets `delivered_at` to current UTC time

### Req-2: Relative Time Parsing

Parse relative time expressions into absolute UTC datetimes. Support common patterns:

| Input | Interpretation |
|-------|---------------|
| `2h` or `2 hours` | now + 2 hours |
| `30m` or `30 minutes` or `30min` | now + 30 minutes |
| `1d` or `1 day` | now + 24 hours |
| `tomorrow 9am` | next 9:00 AM in user's configured timezone |
| `tomorrow` | next day at 9:00 AM (sensible default) |
| `next Monday` | next Monday at 9:00 AM |
| `next Monday 2pm` | next Monday at 14:00 |
| ISO 8601 string | used as-is after parsing |

Implementation:
- Use `chrono` (already a workspace dependency) for datetime math
- User timezone from config (`daemon.timezone`, default `America/Chicago`)
- All storage in UTC — convert on input and output
- Return a clear error if the expression can't be parsed

### Req-3: Tool Definitions

Three new tools registered in `register_tools()`:

**`set_reminder`** — Claude calls this when user asks to be reminded.
```json
{
  "name": "set_reminder",
  "description": "Set a reminder that will fire as a message at the specified time. Use for 'remind me to...' requests.",
  "input_schema": {
    "type": "object",
    "properties": {
      "message": {
        "type": "string",
        "description": "What to remind about (e.g. 'check the CT deploy')"
      },
      "due_at": {
        "type": "string",
        "description": "When to fire — ISO 8601 datetime OR relative like '2h', '30m', 'tomorrow 9am', 'next Monday'"
      },
      "channel": {
        "type": "string",
        "description": "Channel to send reminder to (default: channel the request came from)"
      }
    },
    "required": ["message", "due_at"]
  }
}
```

**`list_reminders`** — Show active reminders.
```json
{
  "name": "list_reminders",
  "description": "List all active (unfired, uncancelled) reminders with their IDs and due times.",
  "input_schema": {
    "type": "object",
    "properties": {},
    "required": []
  }
}
```

**`cancel_reminder`** — Cancel a reminder by ID.
```json
{
  "name": "cancel_reminder",
  "description": "Cancel an active reminder by its ID. Returns whether the cancellation succeeded.",
  "input_schema": {
    "type": "object",
    "properties": {
      "id": {
        "type": "integer",
        "description": "The reminder ID to cancel (from list_reminders)"
      }
    },
    "required": ["id"]
  }
}
```

Tool dispatch: All three are immediate (no pending action / confirm flow). `set_reminder` is a
personal timer, not a write operation that needs confirmation.

### Req-4: Reminder Scheduler (Background Task)

A background tokio task spawned from `main.rs` (alongside the existing digest scheduler):

1. Poll SQLite every 30 seconds for due reminders (`get_due_reminders()`)
2. For each due reminder:
   a. Send message to the reminder's channel: `"Reminder: {message}"`
   b. Mark as delivered (`mark_delivered(id)`)
   c. Log success/failure via tracing
3. On send failure: log the error but do NOT retry immediately — the reminder stays undelivered
   and will be picked up on the next poll cycle
4. The task takes a clone of the channel registry (`HashMap<String, Arc<dyn Channel>>`) and a
   reference to the reminder store

Survival guarantee: since reminders are in SQLite, they survive daemon restarts. On restart, the
scheduler picks up any overdue reminders on its first poll cycle and fires them immediately.

### Req-5: Channel-Aware Delivery

The reminder fires to the channel where it was set. The `channel` field is populated from the
trigger's channel name (e.g., `"telegram"`, `"discord"`). If the channel is unavailable at fire
time (e.g., daemon restarted without that channel configured), log a warning and leave the
reminder undelivered for the next cycle.

## Scope
- **IN**: SQLite table + store methods, relative time parsing, three tools (`set_reminder`, `list_reminders`, `cancel_reminder`), background polling task, channel-aware delivery
- **OUT**: Recurring/repeating reminders (only one-shot), snooze functionality, reminder editing (cancel and re-create instead), notification sounds, timezone auto-detection from Telegram

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/reminders.rs` | New: `ReminderStore` (init table, CRUD methods), `Reminder` struct, relative time parsing |
| `crates/nv-daemon/src/tools.rs` | Add 3 tool definitions (`set_reminder`, `list_reminders`, `cancel_reminder`) + dispatch in `execute_tool()` and `execute_tool_send()` |
| `crates/nv-daemon/src/main.rs` | Add `mod reminders`, init `ReminderStore` (reuse messages.db path), spawn reminder scheduler task, add to `SharedDeps` |
| `crates/nv-daemon/src/worker.rs` | Add `reminder_store` field to `SharedDeps` |
| `crates/nv-core/src/lib.rs` | Add `timezone` field to daemon config (optional, default `America/Chicago`) |

## Risks
| Risk | Mitigation |
|------|-----------|
| 30s poll granularity means reminders fire up to 30s late | Acceptable for personal reminders. Users don't expect sub-second precision from "remind me in 2 hours". |
| Relative time parsing edge cases (DST, ambiguous "next Monday") | Use chrono-tz for timezone-aware math. "next Monday" always means the upcoming Monday. Ambiguous inputs get a clear error. |
| SQLite contention with MessageStore on same db file | rusqlite `Connection` is not shared across threads — each store opens its own connection. SQLite handles multi-connection writes via WAL mode (already enabled for messages.db). |
| Channel unavailable at fire time | Log warning, leave undelivered. Next poll retries. If channel is permanently gone, reminder stays in limbo — user can cancel via `cancel_reminder`. |
