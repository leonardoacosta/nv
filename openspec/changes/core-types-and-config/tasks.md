# Tasks: core-types-and-config

## Dependencies

- cargo-workspace-scaffold (spec-1)

## Tasks

### Config

- [x] Define `Config` struct with nested sections: `AgentConfig`, `TelegramConfig`, `JiraConfig`, `NexusConfig` (with `NexusAgent` vec), `DaemonConfig` — all with `serde::Deserialize`
- [x] Define default functions: `default_think() -> true`, `default_digest_interval() -> 60`, `default_health_port() -> 8400`
- [x] Implement `Config::load()` — reads from `~/.nv/nv.toml`, returns `anyhow::Result<Config>`
- [x] Implement `Config::load_from(path: PathBuf)` — reads from explicit path (for testing)
- [x] Implement `Config::default_path()` — resolves `$HOME/.nv/nv.toml`
- [x] Define `Secrets` struct: `anthropic_api_key` (required), `telegram_bot_token`, `jira_api_token`, `jira_username` (all optional)
- [x] Implement `Secrets::from_env()` — reads from environment variables, fails if `ANTHROPIC_API_KEY` missing

### Core Types

- [x] Define `InboundMessage` struct: id, channel, sender, content, timestamp (`DateTime<Utc>`), thread_id (Option), metadata (`serde_json::Value`) — derive Serialize + Deserialize
- [x] Define `OutboundMessage` struct: channel, content, reply_to (Option), keyboard (Option) — derive Serialize + Deserialize
- [x] Define `InlineKeyboard` struct: rows (`Vec<Vec<InlineButton>>`) and `InlineButton` struct: text, callback_data
- [x] Define `Trigger` enum: `Message(InboundMessage)`, `Cron(CronEvent)`, `NexusEvent(SessionEvent)`, `CliCommand(CliRequest)`
- [x] Define `CronEvent` enum: `Digest`, `MemoryCleanup`
- [x] Define `SessionEvent` struct: agent_name, session_id, event_type (`SessionEventType`), details (Option)
- [x] Define `SessionEventType` enum: `Started`, `Completed`, `Failed`, `Progress`
- [x] Define `CliRequest` struct: command (`CliCommand`), response_tx (`Option<oneshot::Sender<String>>`)
- [x] Define `CliCommand` enum: `Status`, `Ask(String)`, `DigestNow`
- [x] Define `AgentResponse` enum: `Reply { channel, content, reply_to, keyboard }`, `Action(PendingAction)`, `Digest { content, suggested_actions }`, `QueryAnswer(String)`, `NoOp`
- [x] Define `PendingAction` struct: id (`Uuid`), description, action_type (`ActionType`), payload (`serde_json::Value`), status (`ActionStatus`), created_at (`DateTime<Utc>`)
- [x] Define `ActionType` enum: `JiraCreate`, `JiraTransition`, `JiraAssign`, `JiraComment`, `ChannelReply`
- [x] Define `ActionStatus` enum: `Pending`, `Approved`, `Rejected`, `Executed`, `Failed`

### Channel Trait

- [x] Define `Channel` async trait in `channel.rs`: `name() -> &str`, `connect()`, `poll_messages()`, `send_message()`, `disconnect()` — all returning `anyhow::Result`
- [x] Add `async-trait` attribute macro, require `Send + Sync`

### Module Wiring

- [x] Update `lib.rs` with `pub use` re-exports: `Config`, `Secrets` from config; all types from types; `Channel` from channel
- [x] Add `tokio` dependency to nv-core `Cargo.toml` (needed for `oneshot::Sender` in `CliRequest`)

### Unit Tests

- [x] Test: parse valid TOML string into `Config` (all sections populated)
- [x] Test: parse minimal TOML (only required `[agent]` section) — optional sections are `None`
- [x] Test: parse TOML with missing `model` field fails with descriptive error
- [x] Test: `Config::load_from` with nonexistent path returns error
- [x] Test: `Secrets::from_env()` succeeds when `ANTHROPIC_API_KEY` is set
- [x] Test: `Secrets::from_env()` fails when `ANTHROPIC_API_KEY` is missing
- [x] Test: `InboundMessage` serialize/deserialize round-trip preserves all fields
- [x] Test: `OutboundMessage` with `InlineKeyboard` serializes to expected JSON structure
- [x] Test: `PendingAction` defaults with `ActionStatus::Pending` and valid UUID

### Verify

- [x] `cargo build` passes for all workspace members
- [x] `cargo test -p nv-core` — all unit tests pass
- [x] `cargo clippy` passes with no warnings
