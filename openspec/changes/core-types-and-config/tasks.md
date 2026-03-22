# Tasks: core-types-and-config

## Dependencies

- cargo-workspace-scaffold (spec-1)

## Tasks

### Config

- [ ] Define `Config` struct with nested sections: `AgentConfig`, `TelegramConfig`, `JiraConfig`, `NexusConfig` (with `NexusAgent` vec), `DaemonConfig` — all with `serde::Deserialize`
- [ ] Define default functions: `default_think() -> true`, `default_digest_interval() -> 60`, `default_health_port() -> 8400`
- [ ] Implement `Config::load()` — reads from `~/.nv/nv.toml`, returns `anyhow::Result<Config>`
- [ ] Implement `Config::load_from(path: PathBuf)` — reads from explicit path (for testing)
- [ ] Implement `Config::default_path()` — resolves `$HOME/.nv/nv.toml`
- [ ] Define `Secrets` struct: `anthropic_api_key` (required), `telegram_bot_token`, `jira_api_token`, `jira_username` (all optional)
- [ ] Implement `Secrets::from_env()` — reads from environment variables, fails if `ANTHROPIC_API_KEY` missing

### Core Types

- [ ] Define `InboundMessage` struct: id, channel, sender, content, timestamp (`DateTime<Utc>`), thread_id (Option), metadata (`serde_json::Value`) — derive Serialize + Deserialize
- [ ] Define `OutboundMessage` struct: channel, content, reply_to (Option), keyboard (Option) — derive Serialize + Deserialize
- [ ] Define `InlineKeyboard` struct: rows (`Vec<Vec<InlineButton>>`) and `InlineButton` struct: text, callback_data
- [ ] Define `Trigger` enum: `Message(InboundMessage)`, `Cron(CronEvent)`, `NexusEvent(SessionEvent)`, `CliCommand(CliRequest)`
- [ ] Define `CronEvent` enum: `Digest`, `MemoryCleanup`
- [ ] Define `SessionEvent` struct: agent_name, session_id, event_type (`SessionEventType`), details (Option)
- [ ] Define `SessionEventType` enum: `Started`, `Completed`, `Failed`, `Progress`
- [ ] Define `CliRequest` struct: command (`CliCommand`), response_tx (`Option<oneshot::Sender<String>>`)
- [ ] Define `CliCommand` enum: `Status`, `Ask(String)`, `DigestNow`
- [ ] Define `AgentResponse` enum: `Reply { channel, content, reply_to, keyboard }`, `Action(PendingAction)`, `Digest { content, suggested_actions }`, `QueryAnswer(String)`, `NoOp`
- [ ] Define `PendingAction` struct: id (`Uuid`), description, action_type (`ActionType`), payload (`serde_json::Value`), status (`ActionStatus`), created_at (`DateTime<Utc>`)
- [ ] Define `ActionType` enum: `JiraCreate`, `JiraTransition`, `JiraAssign`, `JiraComment`, `ChannelReply`
- [ ] Define `ActionStatus` enum: `Pending`, `Approved`, `Rejected`, `Executed`, `Failed`

### Channel Trait

- [ ] Define `Channel` async trait in `channel.rs`: `name() -> &str`, `connect()`, `poll_messages()`, `send_message()`, `disconnect()` — all returning `anyhow::Result`
- [ ] Add `async-trait` attribute macro, require `Send + Sync`

### Module Wiring

- [ ] Update `lib.rs` with `pub use` re-exports: `Config`, `Secrets` from config; all types from types; `Channel` from channel
- [ ] Add `tokio` dependency to nv-core `Cargo.toml` (needed for `oneshot::Sender` in `CliRequest`)

### Unit Tests

- [ ] Test: parse valid TOML string into `Config` (all sections populated)
- [ ] Test: parse minimal TOML (only required `[agent]` section) — optional sections are `None`
- [ ] Test: parse TOML with missing `model` field fails with descriptive error
- [ ] Test: `Config::load_from` with nonexistent path returns error
- [ ] Test: `Secrets::from_env()` succeeds when `ANTHROPIC_API_KEY` is set
- [ ] Test: `Secrets::from_env()` fails when `ANTHROPIC_API_KEY` is missing
- [ ] Test: `InboundMessage` serialize/deserialize round-trip preserves all fields
- [ ] Test: `OutboundMessage` with `InlineKeyboard` serializes to expected JSON structure
- [ ] Test: `PendingAction` defaults with `ActionStatus::Pending` and valid UUID

### Verify

- [ ] `cargo build` passes for all workspace members
- [ ] `cargo test -p nv-core` — all unit tests pass
- [ ] `cargo clippy` passes with no warnings
