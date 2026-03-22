# Implementation Tasks

## Phase 1: Health State and Endpoint

- [x] [1.1] Create `crates/nv-daemon/src/health.rs` — `HealthState` struct with `started_at: Instant`, `triggers_processed: AtomicU64`, `channel_status: RwLock<HashMap<String, ChannelStatus>>`, `last_digest_at: RwLock<Option<DateTime<Utc>>>`. Methods: `new()`, `record_trigger()`, `update_channel(name, status)`, `update_last_digest(timestamp)`, `to_health_response()` returning serializable `HealthResponse` struct [owner:api-engineer]
- [x] [1.2] Add `GET /health` to `crates/nv-daemon/src/http.rs` — handler calls `health_state.to_health_response()`, returns JSON with `status`, `uptime_secs`, `version` (from `env!("CARGO_PKG_VERSION")`), `channels` map, `last_digest_at`, `triggers_processed`. Accept `Arc<HealthState>` via axum state (depends: 1.1) [owner:api-engineer]
- [x] [1.3] Wire `HealthState` into existing daemon components — create `Arc<HealthState>` in `main.rs`, pass to HTTP server router, agent loop (for `record_trigger()`), channel listeners (for `update_channel()`), digest sender (for `update_last_digest()`) (depends: 1.1) [owner:api-engineer]

## Phase 2: Log Rotation

- [x] [2.1] Add `tracing-appender = "0.2"` to `[workspace.dependencies]` in root `Cargo.toml` and `tracing-appender = { workspace = true }` to `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [x] [2.2] Replace tracing init in `crates/nv-daemon/src/main.rs` — create `tracing_appender::rolling::daily("~/.nv/logs", "nv")` rolling file appender. Build subscriber with two layers: stdout `fmt::layer()` + file appender layer via `tracing_subscriber::fmt::layer().with_writer(non_blocking_appender)`. Use `tracing_appender::non_blocking()` for async file writes. Create log directory with `fs::create_dir_all()` on startup (depends: 2.1) [owner:api-engineer]
- [x] [2.3] Implement log file cleanup — on startup, list files matching `~/.nv/logs/nv.*`, sort by modification time, delete all but the 5 most recent. Log the count of files removed at info level (depends: 2.2) [owner:api-engineer]

## Phase 3: Graceful Shutdown

- [x] [3.1] Create `crates/nv-daemon/src/shutdown.rs` — `ShutdownCoordinator` struct holding `mpsc::Sender<Trigger>` (to drop), `Vec<JoinHandle>` (agent loop + channel tasks), and channel disconnect handles. `signal_listener()` async fn waits on both `ctrl_c()` and `tokio::signal::unix::signal(SignalKind::terminate())` [owner:api-engineer]
- [x] [3.2] Implement shutdown sequence in `ShutdownCoordinator::run()` — on signal: log "Shutdown signal received", drop mpsc sender, await agent loop JoinHandle with 10s timeout, flush state files to `~/.nv/state/`, call `disconnect()` on each channel, send `sd_notify(STOPPING=1)`, log "NV daemon stopped cleanly" (depends: 3.1, 4.1) [owner:api-engineer]
- [x] [3.3] Wire shutdown coordinator into `crates/nv-daemon/src/main.rs` — replace existing `ctrl_c()` handler with `ShutdownCoordinator`. Pass all spawned task JoinHandles and the trigger sender to the coordinator. Spawn coordinator as the last task before awaiting (depends: 3.2) [owner:api-engineer]

## Phase 4: sd-notify Integration

- [x] [4.1] Add `sd-notify = "0.4"` to `[workspace.dependencies]` in root `Cargo.toml` and `sd-notify = { workspace = true }` to `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [x] [4.2] Send `sd_notify::notify(false, &[NotifyState::Ready])` in `crates/nv-daemon/src/main.rs` after all channel listeners are spawned and connected, agent loop is running, and HTTP server is bound (depends: 4.1) [owner:api-engineer]
- [x] [4.3] Spawn watchdog task — every 30 seconds (half of `WatchdogSec=60`), run internal health check via `health_state.to_health_response()`. If status is "ok", send `sd_notify::notify(false, &[NotifyState::Watchdog])`. If not ok, skip the ping and log a warning — systemd will restart after 60s of missed pings (depends: 4.1, 1.1) [owner:api-engineer]

## Phase 5: systemd Unit File and Install Script

- [x] [5.1] Replace `deploy/nv.service` placeholder with production unit — `Type=notify`, `User=nyaptor`, `ExecStart=%h/.local/bin/nv-daemon`, `Restart=on-failure`, `RestartSec=5s`, `WatchdogSec=60`, `TimeoutStopSec=30`, `EnvironmentFile=-%h/.config/nv/env`, `LimitNOFILE=4096`, `MemoryMax=512M`, `WantedBy=default.target` [owner:api-engineer]
- [x] [5.2] Create `deploy/install.sh` — `set -euo pipefail`. Steps: `cargo build --release -p nv-daemon -p nv-cli`, `mkdir -p ~/.local/bin`, copy binaries, `mkdir -p ~/.config/systemd/user`, copy service file, `systemctl --user daemon-reload`, `systemctl --user enable nv.service`, `systemctl --user restart nv.service`, wait 3s, verify with `systemctl --user is-active nv.service` and `curl -sf http://localhost:8400/health`. Print success/failure summary. `chmod +x` the script [owner:api-engineer]

## Phase 6: CLI Status Command

- [x] [6.1] Create `crates/nv-cli/src/commands/status.rs` — `nv status` subcommand. HTTP GET to `http://localhost:{health_port}/health`, parse JSON `HealthResponse`. Run `systemctl --user is-active nv.service` for unit state. Format combined output: daemon state, version, health status, channel list with status, last digest time (relative), trigger count. Non-zero exit code if daemon is not running [owner:api-engineer]
- [x] [6.2] Add fallback display — if HTTP request to `/health` fails (connection refused), show `NV Daemon: stopped` with systemd status only. If systemd also reports inactive, show `NV Daemon: not installed` if service file missing or `NV Daemon: stopped` otherwise (depends: 6.1) [owner:api-engineer]
- [x] [6.3] Register status command in `crates/nv-cli/src/main.rs` — replace the placeholder `Commands::Status` match arm with the real implementation from `commands/status.rs`. Add `reqwest` (blocking client) and `serde_json` to nv-cli `Cargo.toml` if not already present (depends: 6.1) [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Health | `cargo build -p nv-daemon` — health module compiles, endpoint registered in router |
| 2 Logs | `cargo build -p nv-daemon` — rolling appender initializes, log directory created |
| 3 Shutdown | `cargo build -p nv-daemon` — shutdown coordinator compiles with SIGTERM + ctrl_c handling |
| 4 sd-notify | `cargo build -p nv-daemon` — sd_notify calls compile, watchdog task spawns |
| 5 Deploy | `deploy/install.sh` runs successfully — binaries installed, service enabled and started |
| 6 CLI | `nv status` shows daemon health from `/health` endpoint + systemd status |
| **Final** | `systemctl --user start nv` → daemon running → `nv status` shows ok → Telegram receives test message |
