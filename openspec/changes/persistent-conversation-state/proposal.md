# Proposal: Persistent Conversation State

## Change ID
`persistent-conversation-state`

## Summary

Replace the in-memory `ConversationStore` (10-minute timeout, cleared on daemon restart) with a
SQLite-backed persistent conversation store. Nova maintains multi-turn conversation history across
daemon restarts, with configurable TTL, thread-aware session grouping, and automatic summarization
when conversations exceed the token budget.

## Context
- Phase: Wave 3c
- Depends on: `native-tool-use-protocol` (direct Anthropic API required for token counting)
- Extends: `crates/nv-daemon/src/conversation.rs` (replaces in-memory ConversationStore)
- Related: `crates/nv-daemon/src/messages.rs` (existing SQLite pattern to follow), `crates/nv-daemon/src/worker.rs` (ConversationStore load/push call sites)
- Beads: nv-93d (conversation-persistence)

## Motivation

The current `ConversationStore` is a `Vec<(Message, Message)>` in process memory. Two failure
modes break conversational continuity:

1. **Daemon restart** — systemd restarts Nova after updates, crashes, or reboots. All conversation
   context is lost. Nova greets returning users as strangers.
2. **10-minute timeout** — any pause longer than `SESSION_TIMEOUT` clears history. A 15-minute
   Jira rabbit-hole invalidates the conversation context from before it started.

The existing `MessageStore` in `messages.rs` already uses `rusqlite` with versioned migrations and
WAL mode. Persistent conversation state follows the same pattern, adding a `conversations` table
to `messages.db` that serializes API-level `Message` objects as JSON blobs.

Benefits:
1. **Restart survival** — conversation history survives daemon restarts
2. **Configurable TTL** — 24-hour default, operator-tunable via `daemon.conversation_ttl_hours`
3. **Thread awareness** — Telegram forum threads and topics get isolated conversation contexts
4. **Token budget enforcement** — when history exceeds a configurable token budget, oldest turns
   are summarized and replaced with a compact summary block
5. **Zero new dependencies** — `rusqlite` is already in the workspace

## Requirements

### Req-1: conversations Table Migration

Add a new migration (version N+1) to `messages.db` via `messages_migrations()` in `messages.rs`:

```sql
CREATE TABLE IF NOT EXISTS conversations (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    channel   TEXT    NOT NULL,
    thread_id TEXT    NOT NULL DEFAULT '',
    messages  TEXT    NOT NULL,          -- JSON: Vec<Message>
    created_at TEXT   NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT   NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT   NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_channel_thread
    ON conversations(channel, thread_id);
CREATE INDEX IF NOT EXISTS idx_conversations_expires_at
    ON conversations(expires_at);
```

`thread_id` is empty string for non-threaded channels (DMs, CLI, iMessage). Telegram forum
threads populate it with the Telegram `message_thread_id` cast to string.

The `UNIQUE INDEX` on `(channel, thread_id)` means there is exactly one active conversation per
channel+thread combination at any time. `INSERT OR REPLACE` upserts the row on each push.

### Req-2: PersistentConversationStore

Replace `ConversationStore` with `PersistentConversationStore` in `conversation.rs`. The struct
wraps a `rusqlite::Connection` and the channel/thread identity:

```rust
pub struct PersistentConversationStore {
    conn: Arc<Mutex<Connection>>,
    channel: String,
    thread_id: String,
    ttl_hours: u64,
}
```

Public interface mirrors the existing `ConversationStore` to minimize call-site changes:

| Method | Behaviour |
|--------|-----------|
| `new(conn, channel, thread_id, ttl_hours)` | Constructor; does NOT clear expired rows (lazy) |
| `push(user_msg, assistant_msg) -> Result<()>` | Appends turn, truncates tool results, upserts row, updates `updated_at` and `expires_at` |
| `load() -> Result<Vec<Message>>` | Reads current row; returns empty vec if expired or missing; does NOT mutate `last_activity` |
| `clear() -> Result<()>` | Deletes the row for this channel+thread (used by `/reset` command) |

`push` serializes the full `Vec<(Message, Message)>` turns list as JSON into the `messages`
column. `load` deserializes it. No separate rows per turn — one JSON blob per conversation.

Expiry is checked on `load`: if `expires_at < datetime('now')`, delete the row and return empty.
This is the only place expired rows are cleaned up (lazy expiry). A background sweep is out of
scope.

### Req-3: Thread-Aware Routing in Worker

Workers receive `channel` and `thread_id` from the task. The `SharedDeps` currently holds a
single `conversation_store: Arc<Mutex<ConversationStore>>`. Replace it with the SQLite connection:

```rust
pub conversation_db: Arc<Mutex<Connection>>,
pub conversation_ttl_hours: u64,
```

Each `Worker::run` constructs a scoped `PersistentConversationStore` for its
`(channel, thread_id)` pair:

```rust
let conv_store = PersistentConversationStore::new(
    Arc::clone(&deps.conversation_db),
    channel.clone(),
    thread_id.clone(),
    deps.conversation_ttl_hours,
);
```

This replaces the lock-then-call pattern on `deps.conversation_store`.

The `channel` is already present in the task triggers. The `thread_id` is derived from the
trigger: for Telegram triggers, use `message_thread_id` if present (forum threads), otherwise
empty string. For all other channels, empty string.

### Req-4: Configurable TTL

Add `conversation_ttl_hours` to `DaemonConfig` in `nv-core/src/config.rs`:

```rust
/// Conversation TTL in hours (default: 24). Conversations with no activity
/// for this duration are expired on next load.
#[serde(default = "default_conversation_ttl_hours")]
pub conversation_ttl_hours: u64,
```

```rust
fn default_conversation_ttl_hours() -> u64 { 24 }
```

Wire through `main.rs` into `SharedDeps.conversation_ttl_hours`.

### Req-5: Token Budget Summarization

When `load()` returns a history that exceeds a character budget (kept as a constant
`MAX_HISTORY_CHARS = 50_000`, consistent with the existing in-memory value), auto-summarize the
oldest turns before returning.

Summarization strategy — no API call required at this stage:

1. Identify the oldest N turns that push total chars over budget.
2. Extract the text content of those turns (ignore tool use/results).
3. Compress them into a synthetic `user` + `assistant` turn pair using a fixed template:

```
user:      "[Summary of earlier conversation]"
assistant: "<summary>\n{concatenated text content of compressed turns}\n</summary>"
```

4. Replace the compressed turns with this single synthetic pair.
5. Persist the updated (summarized) turns back to SQLite.

This keeps the conversation storable without requiring a live API call. The `native-tool-use-protocol`
dependency is listed because token counting via the direct API (rather than character estimates)
is the preferred future upgrade path — but character-based summarization ships first and works
without it.

### Req-6: Backward Compatibility

On first startup after this change, existing daemon instances have an empty `conversations` table.
No data migration is needed: the in-memory store was ephemeral by definition. The migration runs
automatically via `rusqlite_migration` version tracking.

Old `ConversationStore` is fully deleted. No deprecated wrapper. No fallback path. The migration
is a clean replacement.

## Scope
- **IN**: `conversations` table migration, `PersistentConversationStore`, thread-aware routing via `channel`+`thread_id`, configurable TTL, character-budget summarization, deletion of old in-memory `ConversationStore`, config field `conversation_ttl_hours`
- **OUT**: Per-turn expiry granularity (whole conversation expires atomically), background expiry sweep, token-count-based summarization via direct API (future upgrade when `native-tool-use-protocol` lands), cross-channel conversation continuity, conversation export/import tools

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/messages.rs` | Add `conversations` table migration (version N+1) |
| `crates/nv-daemon/src/conversation.rs` | Replace `ConversationStore` with `PersistentConversationStore`; update `truncate_history` |
| `crates/nv-daemon/src/worker.rs` | Replace `conversation_store: Arc<Mutex<ConversationStore>>` with `conversation_db: Arc<Mutex<Connection>>` + `conversation_ttl_hours: u64` in `SharedDeps`; update `Worker::run` load/push call sites |
| `crates/nv-core/src/config.rs` | Add `conversation_ttl_hours` to `DaemonConfig` with default 24 |
| `crates/nv-daemon/src/main.rs` | Open conversation DB connection (reuse `messages.db` connection or open a second handle), init `SharedDeps` fields, remove old `ConversationStore::new()` |

## Risks

| Risk | Mitigation |
|------|-----------|
| SQLite contention between conversation writes and message log writes | Both use WAL mode; concurrent readers/writers are safe. Conversation write path holds the lock for <1ms. |
| JSON blob grows unbounded before summarization kicks in | `MAX_HISTORY_TURNS = 20` cap (unchanged) plus `MAX_HISTORY_CHARS = 50_000` trigger summarization. Practical ceiling ~200KB per conversation blob, well within SQLite limits. |
| Thread ID extraction wrong for non-forum Telegram chats | Empty string default is safe — all DMs share one conversation context, matching current behaviour. |
| Old `messages.db` missing `conversations` table | `rusqlite_migration` checks `PRAGMA user_version` and runs only new migrations; fully safe. |
