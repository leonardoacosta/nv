# Tasks: harden-telegram-nexus

## Dependencies

- telegram-channel (MVP spec)
- nexus-integration (MVP spec)
- jira-integration (for Create Bug callback flow)

## Tasks

### Telegram Integration Test

- [x] Add `integration` feature to `crates/nv-daemon/Cargo.toml` under `[features]` -- no extra dependencies, just a feature gate for integration tests [owner:api-engineer]
- [x] Create integration test in `crates/nv-daemon/src/telegram/mod.rs` (behind `#[cfg(feature = "integration")]` + env var gate `NV_TELEGRAM_INTEGRATION_TEST=1`): read `TELEGRAM_BOT_TOKEN` and `NV_TEST_CHAT_ID` from env, create `TelegramClient`, call `get_me()` to verify token, send echo message to chat, verify both responses succeed [owner:api-engineer]

### Nexus Error Callback Wiring

- [x] Wire Nexus error callback routing in Telegram handler / agent loop (`crates/nv-daemon/src/agent.rs`) -- match `[callback]` data with prefix `nexus_err:view:{session_id}`: query session via `NexusClient::query_session`, format full error details, send as Telegram reply message [owner:api-engineer]
- [x] Wire `nexus_err:bug:{session_id}` callback -- query session via `NexusClient::query_session`, create `PendingAction` with `JiraActionType::Create` pre-filled (project from session, title: "Session error: {project}", description: error message + session context), save action, send Jira confirmation keyboard via Telegram [owner:api-engineer]
- [x] Add `format_session_error_detail(session)` helper in `crates/nv-daemon/src/nexus/notify.rs` -- format full error text for the "View Error" callback reply (session id, project, agent, error message, duration, timestamp) [owner:api-engineer]
- [x] Add `create_bug_from_session_error(session)` helper in `crates/nv-daemon/src/nexus/notify.rs` -- build a `PendingAction` with pre-filled JiraCreate params from session error context [owner:api-engineer]

### Verify

- [x] `cargo build` passes for all workspace members
- [x] `cargo test -p nv-daemon` -- all existing unit tests pass (441 passed)
- [ ] `cargo test -p nv-daemon --features integration` -- Telegram integration test passes (requires `TELEGRAM_BOT_TOKEN` and `NV_TEST_CHAT_ID` env vars)
- [x] `cargo clippy` passes with no warnings
- [ ] Manual gate: set `TELEGRAM_BOT_TOKEN` + chat_id, run daemon, send "hello" on Telegram, bot echoes back [user]
- [ ] Manual gate: trigger Nexus session error notification, tap "View Error" on inline keyboard -- full error details displayed as reply. Tap "Create Bug" -- Jira draft shown with pre-filled session error context, confirm creates issue [user]
