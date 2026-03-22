# Package and Deploy as systemd Service

| Field | Value |
|-------|-------|
| Spec | `systemd-deploy` |
| Priority | P2 |
| Type | task |
| Effort | small |
| Wave | 5 |

## Context

NV needs to run as a long-lived daemon on the homelab, managed by systemd. The scaffold spec (spec-1) created a placeholder `deploy/nv.service` unit file, but deploying the daemon requires a complete deployment package: a production-ready systemd unit file with Doppler secret injection, an install script that builds and installs binaries, a health check HTTP endpoint for liveness monitoring, structured log rotation, and CLI integration so `nv status` reports daemon health.

The daemon already has an HTTP server on `config.daemon.health_port` (default 8400) introduced by the proactive-digest (spec-7) and context-query (spec-8) specs for `/digest` and `/ask` endpoints. The health check endpoint (`GET /health`) is added to this same server. The `nv status` CLI command connects to this endpoint and also reads `systemctl status nv` for a combined view.

Graceful shutdown is partially implemented (spec-1 scaffolded `ctrl_c()`, spec-3 added channel sender drops). This spec adds proper SIGTERM handling — draining the mpsc trigger channel, saving state files, and exiting cleanly so systemd sees a clean stop.

## User Stories

- **Operations**: `systemctl start nv` boots the daemon, which connects to Telegram, Nexus, and begins processing triggers
- **Monitoring**: `nv status` shows daemon uptime, connected channels, last digest time, and active Nexus sessions — combining the `/health` JSON response with systemd unit status
- **Reliability**: On crash, systemd restarts the daemon within 5 seconds. Watchdog ensures the daemon hasn't hung
- **Deployment**: `deploy/install.sh` builds from source, copies binaries to `~/.local/bin/`, installs the systemd unit, and starts the service

## Proposed Changes

### systemd Unit File

- `deploy/nv.service`: Replace the placeholder from spec-1 with a production-ready unit file:
  ```ini
  [Unit]
  Description=NV Master Agent Harness
  After=network-online.target
  Wants=network-online.target

  [Service]
  Type=notify
  User=nyaptor
  ExecStart=%h/.local/bin/nv-daemon
  Restart=on-failure
  RestartSec=5s
  WatchdogSec=60
  TimeoutStopSec=30
  EnvironmentFile=-%h/.config/nv/env
  Environment=RUST_LOG=info
  # Resource limits
  LimitNOFILE=4096
  MemoryMax=512M

  [Install]
  WantedBy=default.target
  ```
  Key differences from the scaffold placeholder:
  - `Type=notify` instead of `simple` — daemon sends `sd_notify(READY=1)` after all channels connect, so systemd knows when the service is truly ready. Uses the `sd-notify` crate.
  - `ExecStart=%h/.local/bin/nv-daemon` — user-local install path using `%h` (home directory specifier), installed via `systemctl --user`
  - `WantedBy=default.target` — user service, not system-wide
  - `TimeoutStopSec=30` — allows graceful shutdown to complete (drain mpsc, save state)
  - `LimitNOFILE=4096` — sufficient for gRPC connections + HTTP + Telegram polling
  - `MemoryMax=512M` — prevents runaway memory from affecting the homelab
  - `EnvironmentFile=-%h/.config/nv/env` — optional env file for Doppler-injected secrets (the `-` prefix means no error if file is missing, allowing Doppler `doppler run` as an alternative)

### Install Script

- `deploy/install.sh`: Bash script for building and deploying:
  ```bash
  #!/usr/bin/env bash
  set -euo pipefail

  INSTALL_DIR="${HOME}/.local/bin"
  SERVICE_DIR="${HOME}/.config/systemd/user"
  ```
  Steps:
  1. `cargo build --release -p nv-daemon -p nv-cli` — builds both binaries
  2. Copy `target/release/nv-daemon` and `target/release/nv-cli` to `~/.local/bin/`
  3. Copy `deploy/nv.service` to `~/.config/systemd/user/nv.service`
  4. `systemctl --user daemon-reload`
  5. `systemctl --user enable nv.service`
  6. `systemctl --user restart nv.service`
  7. Wait 3 seconds, then check `systemctl --user is-active nv.service` and `curl localhost:8400/health`
  8. Print success or failure summary

  The script is idempotent — safe to re-run after code changes. Uses `restart` (not `start`) so it works for both fresh installs and upgrades.

### Health Check Endpoint

- `crates/nv-daemon/src/http.rs`: Add `GET /health` endpoint to the existing axum HTTP server. Returns JSON:
  ```json
  {
    "status": "ok",
    "uptime_secs": 3600,
    "version": "0.1.0",
    "channels": {
      "telegram": "connected",
      "nexus_homelab": "connected",
      "nexus_macbook": "disconnected"
    },
    "last_digest_at": "2026-03-21T10:00:00Z",
    "triggers_processed": 142
  }
  ```
  The endpoint aggregates state from:
  - Daemon start time (stored as `Instant` in shared state)
  - Channel connection status from each channel listener and Nexus client
  - Last digest timestamp from `state/last-digest.json`
  - Trigger counter (atomic u64 incremented in agent loop)
  - Version from `env!("CARGO_PKG_VERSION")`

  This endpoint is also used for systemd watchdog — the daemon pings `sd_notify(WATCHDOG=1)` on a timer if the health check returns OK internally.

### Health State Struct

- `crates/nv-daemon/src/health.rs`: `HealthState` struct shared across the daemon via `Arc<HealthState>`:
  ```rust
  pub struct HealthState {
      started_at: Instant,
      triggers_processed: AtomicU64,
      channel_status: RwLock<HashMap<String, ChannelStatus>>,
      last_digest_at: RwLock<Option<DateTime<Utc>>>,
  }
  ```
  Methods:
  - `new()` — records start time
  - `record_trigger()` — increment counter
  - `update_channel(name, status)` — called by channel listeners on connect/disconnect
  - `update_last_digest(timestamp)` — called after digest send
  - `to_health_response()` — builds the JSON response struct

### Log Rotation

- `crates/nv-daemon/src/main.rs`: Replace the basic `tracing_subscriber::fmt()` with `tracing-appender` for file-based log rotation:
  - `tracing_appender::rolling::daily(log_dir, "nv")` — creates daily rolling log files in `~/.nv/logs/` with filenames like `nv.2026-03-21`
  - Retention: 5 files max via a cleanup task that runs on startup — lists `~/.nv/logs/nv.*` files, sorts by mtime, deletes all but the 5 most recent
  - Stdout logging preserved via `tracing_subscriber::fmt::layer()` with a tee — logs go to both the rolling file and stdout (so `journalctl --user -u nv` also captures output)
  - Log directory created on startup if it doesn't exist: `fs::create_dir_all(log_dir)`

- `crates/nv-daemon/Cargo.toml`: Add `tracing-appender = { workspace = true }` to dependencies.
- Root `Cargo.toml`: Add `tracing-appender = "0.2"` to `[workspace.dependencies]`.

### Graceful Shutdown

- `crates/nv-daemon/src/shutdown.rs`: Shutdown coordinator:
  - Listens for SIGTERM via `tokio::signal::unix::signal(SignalKind::terminate())` alongside the existing `ctrl_c()` handler
  - On signal received:
    1. Log `"Shutdown signal received, draining..."`
    2. Drop the `mpsc::Sender<Trigger>` — this causes the agent loop to exit when the channel drains
    3. Wait for the agent loop task to complete (via `JoinHandle`)
    4. Save state: flush any pending state to `~/.nv/state/` (channel-state.json, last-digest.json)
    5. Disconnect channels: call `disconnect()` on each channel (Telegram, Nexus)
    6. Log `"NV daemon stopped cleanly"`
    7. Send `sd_notify(STOPPING=1)` before exit
  - `TimeoutStopSec=30` in the unit file ensures systemd sends SIGKILL if this takes too long

### sd-notify Integration

- `crates/nv-daemon/src/main.rs`: After all channel listeners and the agent loop are spawned and connected:
  - Send `sd_notify(READY=1)` — tells systemd the service is fully started
  - Spawn a watchdog task: every `WatchdogSec / 2` (30s), check internal health and send `sd_notify(WATCHDOG=1)` if OK. If health check fails internally, skip the watchdog ping — systemd will restart the service after 60s
- `crates/nv-daemon/Cargo.toml`: Add `sd-notify = "0.4"` to dependencies.
- Root `Cargo.toml`: Add `sd-notify = "0.4"` to `[workspace.dependencies]`.

### CLI Status Command

- `crates/nv-cli/src/commands/status.rs`: Implement the `nv status` subcommand:
  1. HTTP GET to `http://localhost:{health_port}/health` — parse the JSON response
  2. `systemctl --user is-active nv.service` — get systemd unit state
  3. `systemctl --user show nv.service --property=ActiveEnterTimestamp` — get systemd-reported uptime
  4. Format combined output:
     ```
     NV Daemon: running (uptime: 2h 14m)
     Version:   0.1.0
     Health:    ok
     Channels:
       telegram:      connected
       nexus_homelab:  connected
       nexus_macbook:  disconnected
     Last Digest: 14 minutes ago
     Triggers:    142 processed
     ```
  5. If the HTTP request fails, fall back to systemd status only:
     ```
     NV Daemon: stopped (systemd: inactive)
     ```
  6. Non-zero exit code if daemon is not running (useful for scripts)

### Daemon Integration

- `crates/nv-daemon/src/main.rs`: Wire all new components:
  - Create `Arc<HealthState>` and pass to HTTP server, agent loop, and channel listeners
  - Replace shutdown handler with the new `shutdown.rs` coordinator
  - Initialize rolling log appender before anything else
  - Send `sd_notify(READY=1)` after all tasks are spawned and channels connected
  - Spawn watchdog task

## Dependencies

- `nexus-integration` (spec-9) — Nexus connection status reported in health endpoint
- All prior specs (1-9) — the daemon must be feature-complete before deployment packaging makes sense

## Out of Scope

- Doppler service token provisioning (handled manually or via existing `~/dev/co` infrastructure)
- Automatic updates / rolling deploy (manual `deploy/install.sh` re-run for now)
- Container / Docker packaging (systemd user service is the deployment target)
- Monitoring dashboards / alerting (Telegram notifications serve as the alert channel)
- Log shipping to external service (local file logs + journalctl are sufficient for v1)
- Multi-user installation (single-user service under `nyaptor`)
