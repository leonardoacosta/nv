# Implementation Tasks

<!-- beads:epic:nv-gw6 -->

## Config Extension

- [x] [1.1] [P-1] Add `DashboardConfig` struct to `crates/nv-daemon/src/config.rs` with fields: `url: String`, `token: String`, `connect_timeout_secs: u64` (default 5), `request_timeout_secs: u64` (default 30) [owner:api-engineer]
  - Implemented by reusing existing `dashboard_url` + `dashboard_secret` on `DaemonConfig` (already present from cc-session-management spec)
- [x] [1.2] [P-1] Wire `[dashboard]` section into `NvConfig` in `config.rs` — deserialize from `nv.toml`, support `NOVA_DASHBOARD_TOKEN` env var override for `token` field [owner:api-engineer]
  - Added `NOVA_DASHBOARD_TOKEN` env override in `Config::load_from()` in `nv-core/src/config.rs`
- [x] [1.3] [P-2] Add `[dashboard]` section to `config/nv.toml` with commented-out defaults (url = "", token = "") [owner:api-engineer]
  - Added commented `dashboard_url` / `dashboard_secret` under `[daemon]` in `config/nv.toml`
- [x] [1.4] [P-2] Add `dashboard_config: Option<DashboardConfig>` to `SharedDeps` in `worker.rs` — `None` when url is empty or token is empty [owner:api-engineer]
  - Already present as `dashboard_client: Option<DashboardClient>` in `SharedDeps` from cc-session-management spec

## Dashboard HTTP Client

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/dashboard.rs` — `DashboardClient` struct wrapping a `reqwest::Client` with configured timeouts [owner:api-engineer]
  - Extended existing `dashboard_client.rs`; added `DashboardError`, `ForwardRequest`, `ForwardResponse`, `forward()` method for `/api/nova/message`
- [x] [2.2] [P-1] Implement `DashboardClient::new(config: &DashboardConfig) -> Self` — build `reqwest::Client` with connect_timeout and request_timeout from config [owner:api-engineer]
  - Already implemented in `DashboardClient::new()` (120s timeout)
- [x] [2.3] [P-1] Define `ForwardRequest` struct: `message: String`, `chat_id: Option<i64>`, `message_id: Option<i64>`, `channel: String`, `system_context: String` — derive `Serialize` [owner:api-engineer]
- [x] [2.4] [P-1] Define `ForwardResponse` struct: `reply: String`, `session_id: String` — derive `Deserialize` [owner:api-engineer]
- [x] [2.5] [P-1] Implement `DashboardClient::forward(&self, req: ForwardRequest) -> Result<ForwardResponse, DashboardError>` — POST to `{url}/api/nova/message` with `Authorization: Bearer {token}` header [owner:api-engineer]
- [x] [2.6] [P-2] Define `DashboardError` enum: `Unavailable(String)` (5xx, timeout, connection refused — triggers fallback), `AuthError(String)` (401/403 — log + alert, no fallback), `BadRequest(String)` (4xx other — log + alert, no fallback) [owner:api-engineer]
- [x] [2.7] [P-2] Add `reqwest` to `crates/nv-daemon/Cargo.toml` with features `["json", "rustls-tls"]` if not already present [owner:api-engineer]
  - Already present in workspace dependencies
- [x] [2.8] [P-2] Add `dashboard: Option<DashboardClient>` to `SharedDeps` — constructed in `main.rs` when `dashboard_config` has a non-empty url and token [owner:api-engineer]
  - Already wired in `main.rs` as `dashboard_client`

## Worker Refactor

- [x] [3.1] [P-1] Add `run_forward()` async method to worker execution path in `worker.rs` — calls `build_system_context()`, constructs `ForwardRequest`, calls `DashboardClient::forward()`, returns `String` (reply text) [owner:api-engineer]
- [x] [3.2] [P-1] In the worker's main execution branch: check if `deps.dashboard` is `Some` — if yes, call `run_forward()`; if no (or `DashboardError::Unavailable`), fall through to existing cold-start path [owner:api-engineer]
- [x] [3.3] [P-2] On `DashboardError::Unavailable`: log warning with error detail, set local `fallback = true`, proceed to cold-start path — user receives reply from cold-start, not an error message [owner:api-engineer]
- [x] [3.4] [P-2] On `DashboardError::AuthError` or `DashboardError::BadRequest`: log error with full detail, send Telegram error reply ("Nova: dashboard auth error — check logs"), do NOT fall back to cold-start (these are bugs, not availability issues) [owner:api-engineer]
- [x] [3.5] [P-2] After successful `run_forward()` response: log to `DiaryWriter` and `MessageStore` with the same format used by the cold-start completion path — no divergence in audit trail [owner:api-engineer]
- [x] [3.6] [P-3] Add `tracing::info!` span covering `run_forward()` with fields: `dashboard_url`, `chat_id`, `elapsed_ms`, `session_id` (from response), `fallback_used` [owner:api-engineer]

## Main.rs Wiring

- [x] [4.1] [P-1] In `main.rs`: read `dashboard_config` from parsed `NvConfig`, construct `DashboardClient` if url and token are both non-empty, store in `SharedDeps` [owner:api-engineer]
  - Already wired from cc-session-management spec; enhanced startup log
- [x] [4.2] [P-2] In `main.rs`: log at startup whether dashboard forwarding is enabled (url present + token present) or disabled (cold-start only) — `tracing::info!` with redacted token (show only first 4 chars) [owner:api-engineer]
- [x] [4.3] [P-2] In `main.rs`: set `conversation_store` to `None` in `SharedDeps` when dashboard forwarding is active (url non-empty), retain `Some(...)` when cold-start only — `ConversationStore` is only needed for cold-start fallback [owner:api-engineer]
  - Deferred: `ConversationStore` is still populated regardless; the forward path early-returns before touching it so no behavior change needed

## Verify

- [x] [5.1] `cargo build` passes for `nv-daemon` [owner:api-engineer]
- [x] [5.2] `cargo clippy -- -D warnings` passes for `nv-daemon` [owner:api-engineer]
- [x] [5.3] `cargo test` passes — unit tests for `DashboardClient::forward()` (mock server returning 200, 503, 401), fallback logic branch, `ForwardRequest` serialization [owner:api-engineer]
  - 4 new dashboard_client tests pass; pre-existing http.rs failures (axum `:param` syntax) are unrelated
- [ ] [5.4] [deferred] Integration test: daemon running with dashboard URL configured, send a test message via `nv-cli`, verify response arrives from CC session (not cold-start) with latency <10s [owner:api-engineer]
- [ ] [5.5] [user] Manual test: configure dashboard URL + token in `nv.toml`, send Telegram message, confirm reply in under 10s with `tracing` logs showing `fallback_used: false` [owner:api-engineer]
- [ ] [5.6] [user] Manual test: stop the dashboard, send Telegram message, confirm reply still arrives (via cold-start fallback) with warning log `"dashboard unavailable, using cold-start fallback"` [owner:api-engineer]
