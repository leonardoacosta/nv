# Implementation Tasks

<!-- beads:epic:TBD -->

## Config

- [x] [1.1] [P-1] Add `watchdog_interval_secs: u64` to `NexusConfig` in `config.rs` with `#[serde(default = "default_watchdog_interval")]` defaulting to `10`; add the default fn [owner:api-engineer]

## Connection State

- [x] [2.1] [P-1] Add `quarantined_until: Option<Instant>` and `disconnected_since: Option<Instant>` fields to `NexusAgentConnection` in `connection.rs`; initialize both to `None` in `new()` [owner:api-engineer]
- [x] [2.2] [P-1] Add `is_quarantined(&self) -> bool` method — returns true if `quarantined_until` is `Some(t)` and `Instant::now() < t` [owner:api-engineer]
- [x] [2.3] [P-1] Add `quarantine(&mut self)` method — sets `quarantined_until = Some(Instant::now() + Duration::from_secs(300))` [owner:api-engineer]
- [x] [2.4] [P-1] Update `connect()` to clear `quarantined_until = None` and `disconnected_since = None` on successful connect [owner:api-engineer]
- [x] [2.5] [P-1] Update `mark_disconnected()` to set `disconnected_since = Some(Instant::now())` if currently `None` (preserve first-seen timestamp) [owner:api-engineer]
- [x] [2.6] [P-2] Add unit tests for `is_quarantined()`, `quarantine()`, `disconnected_since` lifecycle [owner:api-engineer]

## Health Check RPC

- [x] [3.1] [P-1] Add `health_check(&mut self) -> Result<(), tonic::Status>` method on `NexusAgentConnection` — calls `client.get_health(HealthRequest {})` with `tokio::time::timeout(Duration::from_secs(5), ...)`, updates `last_seen` on success, returns error on timeout/failure [owner:api-engineer]

## Event Stream Recovery

- [x] [4.1] [P-1] Change `spawn_event_streams()` return type to `Vec<JoinHandle<()>>` — collect and return the handles from `tokio::spawn` [owner:api-engineer]
- [x] [4.2] [P-1] Make `run_event_stream()` public (`pub`) so the watchdog can respawn it [owner:api-engineer]

## Watchdog Task

- [x] [5.1] [P-1] Implement `run_watchdog()` async fn in `nexus/client.rs` (or a new `nexus/watchdog.rs`) — takes `NexusClient`, `Arc<HealthState>`, `watchdog_interval_secs`, `Vec<JoinHandle<()>>` (mutable), `trigger_tx`, and channel registry; loops on `tokio::time::interval` [owner:api-engineer]
- [x] [5.2] [P-1] Watchdog loop: for each agent, check `is_quarantined()` — skip if quarantined; if `Connected` call `health_check()` with stale detection (force check if `last_seen` > `3 * interval`); if `Disconnected` call `reconnect()`; if `Reconnecting` skip [owner:api-engineer]
- [x] [5.3] [P-1] On `health_check()` failure: call `mark_disconnected()`, attempt `reconnect()`; if `consecutive_failures >= 10` call `quarantine()` [owner:api-engineer]
- [x] [5.4] [P-1] On successful reconnect: check corresponding `JoinHandle.is_finished()` — if true, respawn event stream via `tokio::spawn(run_event_stream(...))` and replace the handle [owner:api-engineer]
- [x] [5.5] [P-1] On each cycle per agent: call `health_state.update_channel(format!("nexus_{}", name), status)` with `Connected` or `Disconnected` [owner:api-engineer]
- [x] [5.6] [P-1] Handle `GetHealth` returning `Unimplemented` — treat as healthy (connection is alive), log at debug level [owner:api-engineer]

## Telegram Notifications

- [x] [6.1] [P-2] On disconnect detected (first cycle where `disconnected_since` has been set >30s): send Telegram message via channel registry — `"Nexus agent '{name}' disconnected"` [owner:api-engineer]
- [x] [6.2] [P-2] On successful reconnect after notified disconnect: send Telegram message — `"Nexus agent '{name}' reconnected (was down {duration})"` computed from `disconnected_since`; clear `disconnected_since` [owner:api-engineer]
- [x] [6.3] [P-2] Track `disconnect_notified: bool` on connection to avoid re-sending disconnect messages on subsequent watchdog cycles [owner:api-engineer]

## Integration

- [x] [7.1] [P-1] In `main.rs`: after `connect_all()` and `spawn_event_streams()`, spawn the watchdog task with `tokio::spawn(run_watchdog(...))` passing `nexus_client`, `health_state`, `nexus_config.watchdog_interval_secs`, event stream handles, `trigger_tx.clone()`, and channel registry [owner:api-engineer]
- [x] [7.2] [P-1] Export watchdog fn from `nexus/mod.rs` [owner:api-engineer]

## Verify

- [x] [8.1] `cargo build` passes [owner:api-engineer]
- [x] [8.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [8.3] `cargo test` — existing tests pass, new quarantine/stale detection tests pass [owner:api-engineer]
