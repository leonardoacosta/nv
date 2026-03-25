# Implementation Tasks

<!-- beads:epic:nv-93d -->

## 1. Database Migration

- [ ] [1.1] [P-1] Add `conversations` table migration to `messages_migrations()` in `crates/nv-daemon/src/messages.rs` — new `M::up(...)` entry with `CREATE TABLE IF NOT EXISTS conversations`, `UNIQUE INDEX idx_conversations_channel_thread ON conversations(channel, thread_id)`, and `INDEX idx_conversations_expires_at ON conversations(expires_at)` [owner:api-engineer]
- [ ] [1.2] [P-1] Verify migration runs cleanly against an existing `messages.db` (WAL mode, no existing `conversations` table) without data loss to other tables [owner:api-engineer]

## 2. PersistentConversationStore

- [ ] [2.1] [P-1] Rewrite `crates/nv-daemon/src/conversation.rs` — replace `ConversationStore` struct (Vec + Instant) with `PersistentConversationStore` holding `conn: Arc<Mutex<Connection>>`, `channel: String`, `thread_id: String`, `ttl_hours: u64` [owner:api-engineer]
- [ ] [2.2] [P-1] Implement `PersistentConversationStore::new(conn, channel, thread_id, ttl_hours)` — constructor only, no DB I/O [owner:api-engineer]
- [ ] [2.3] [P-1] Implement `push(user_msg: Message, assistant_msg: Message) -> Result<()>` — deserialize existing row (or start empty), append turn, truncate tool results via existing `truncate_tool_results`, trim to `MAX_HISTORY_TURNS`, upsert via `INSERT OR REPLACE INTO conversations (channel, thread_id, messages, updated_at, expires_at) VALUES (...)` [owner:api-engineer]
- [ ] [2.4] [P-1] Implement `load() -> Result<Vec<Message>>` — query row for `(channel, thread_id)`; if missing or `expires_at < datetime('now')` delete and return `vec![]`; otherwise deserialize JSON, run `truncate_history` on result, return flat message list [owner:api-engineer]
- [ ] [2.5] [P-2] Implement `clear() -> Result<()>` — `DELETE FROM conversations WHERE channel = ?1 AND thread_id = ?2` [owner:api-engineer]
- [ ] [2.6] [P-2] Implement character-budget summarization in `push` — after trimming by turn count, if `total_chars > MAX_HISTORY_CHARS`, compress oldest turns into a synthetic `[Summary of earlier conversation]` + `<summary>...</summary>` turn pair before upserting [owner:api-engineer]
- [ ] [2.7] [P-1] Delete the old `ConversationStore` struct, its `Default` impl, and the `SESSION_TIMEOUT` constant — remove all code replaced by the persistent implementation [owner:api-engineer]
- [ ] [2.8] [P-2] Update `truncate_history` free function to remain compatible with the new load path (called in `load()` before returning) [owner:api-engineer]

## 3. Config

- [ ] [3.1] [P-1] Add `fn default_conversation_ttl_hours() -> u64 { 24 }` to `crates/nv-core/src/config.rs` [owner:api-engineer]
- [ ] [3.2] [P-1] Add `#[serde(default = "default_conversation_ttl_hours")] pub conversation_ttl_hours: u64` to `DaemonConfig` struct in `crates/nv-core/src/config.rs` [owner:api-engineer]

## 4. Worker Integration

- [ ] [4.1] [P-1] In `crates/nv-daemon/src/worker.rs`, replace `pub conversation_store: Arc<Mutex<ConversationStore>>` with `pub conversation_db: Arc<Mutex<Connection>>` and `pub conversation_ttl_hours: u64` in `SharedDeps` [owner:api-engineer]
- [ ] [4.2] [P-1] In `Worker::run`, derive `channel` and `thread_id` from the task's trigger batch — for Telegram triggers use `message_thread_id` cast to string if `Some`, otherwise `""` ; for all other channels use `""` [owner:api-engineer]
- [ ] [4.3] [P-1] In `Worker::run`, construct `PersistentConversationStore::new(Arc::clone(&deps.conversation_db), channel, thread_id, deps.conversation_ttl_hours)` replacing the `deps.conversation_store.lock()` pattern at the load site (worker.rs line ~792) [owner:api-engineer]
- [ ] [4.4] [P-1] Update the `push` call site in `Worker::run` (line ~1020) to call `conv_store.push(user_msg, assistant_msg)?` and propagate errors via tracing warn rather than panicking [owner:api-engineer]
- [ ] [4.5] [P-2] Remove the `use crate::conversation::ConversationStore` import; add `use rusqlite::Connection` import in `worker.rs` [owner:api-engineer]

## 5. main.rs Wiring

- [ ] [5.1] [P-1] In `crates/nv-daemon/src/main.rs`, remove the `conversation::ConversationStore::new()` initialization (line ~919) [owner:api-engineer]
- [ ] [5.2] [P-1] Open a second `rusqlite::Connection` to `~/.nv/messages.db` for the conversation store (WAL allows multiple connections), wrap in `Arc<Mutex<Connection>>`, assign to `SharedDeps.conversation_db` [owner:api-engineer]
- [ ] [5.3] [P-1] Read `config.daemon.as_ref().map_or(24, |d| d.conversation_ttl_hours)` and assign to `SharedDeps.conversation_ttl_hours` [owner:api-engineer]

## 6. Tests

- [ ] [6.1] [P-1] Write unit tests in `conversation.rs` using `rusqlite::Connection::open_in_memory()` and running the `conversations` migration manually — test: `push` and `load` round-trip returns correct message pairs [owner:api-engineer]
- [ ] [6.2] [P-1] Test: expired conversation returns empty vec on `load` (set `expires_at` to past timestamp) [owner:api-engineer]
- [ ] [6.3] [P-1] Test: missing conversation (no row) returns empty vec on `load` [owner:api-engineer]
- [ ] [6.4] [P-2] Test: `push` beyond `MAX_HISTORY_TURNS` trims oldest turns [owner:api-engineer]
- [ ] [6.5] [P-2] Test: `push` beyond `MAX_HISTORY_CHARS` triggers summarization and resulting history is under budget [owner:api-engineer]
- [ ] [6.6] [P-2] Test: tool result truncation still fires on `push` (existing behaviour preserved) [owner:api-engineer]
- [ ] [6.7] [P-2] Test: different `(channel, thread_id)` pairs are isolated — push to one does not appear in load of another [owner:api-engineer]
- [ ] [6.8] [P-2] Test: `clear()` removes the row so subsequent `load()` returns empty [owner:api-engineer]

## 7. Verify

- [ ] [7.1] `cargo build --workspace` passes [owner:api-engineer]
- [ ] [7.2] `cargo clippy --workspace -- -D warnings` passes [owner:api-engineer]
- [ ] [7.3] `cargo test --workspace` — all new and existing tests pass [owner:api-engineer]
