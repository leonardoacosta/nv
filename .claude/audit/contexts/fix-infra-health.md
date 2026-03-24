# Context: Fix Infrastructure Health & Polish Issues

## Source: Audit 2026-03-23 (infra domain, ~76/C+ health)

## Problem
Health status never reflects degraded channels, CLI stubs, config validation gaps, hardcoded paths, metrics inaccuracies.

## Findings

### P2 — Health status never reflects degraded channels
- `crates/nv-daemon/src/health.rs:89` — to_health_response() always returns status: "ok"
- A Disconnected channel does not degrade top-level status
- Callers probing /health cannot distinguish "degraded" from "healthy"
- Fix: Check channel statuses; return "degraded" when any channel is Disconnected

### P2 — CLI "not implemented yet" stubs
- `crates/nv-cli/src/main.rs:76` — nv digest (without --now) prints stub
- `crates/nv-cli/src/main.rs:84` — nv config prints stub
- Dev artifacts in released CLI

### P2 — TeamsCheck in nv check but absent from /health?deep=true
- check_services includes TeamsCheck
- to_deep_health_response() does not
- Health endpoint and CLI show different service inventories

### P2 — 3 hardcoded /home/nyaptor paths
- `crates/nv-daemon/src/health_poller.rs:176`
- `crates/nv-daemon/src/claude.rs:255`
- `crates/nv-daemon/src/callbacks.rs:132`
- Portability risk — should use HOME env var or config

### P3 — quiet_start/quiet_end not validated at parse time
- `crates/nv-core/src/config.rs:484`
- Accept any string without HH:MM validation
- Invalid value "25:99" only fails at runtime
- Fix: Parse and validate in Config::load()

### P3 — pending-actions.json race under concurrent workers
- `crates/nv-daemon/src/state.rs:162`
- Unchecked read-modify-write: two concurrent workers can create lost-update
- Low risk for single-user daemon but worth documenting/fixing

### P3 — WatchdogSec=60 in systemd service
- `deploy/nv.service:12`
- Requires daemon to emit WATCHDOG=1 periodically
- Verify sd_notify is being called; if not, daemon gets killed every 60s

### P3 — Teams relay unconditionally enabled
- `deploy/install.sh:115`
- Enabled even without TEAMS_WEBHOOK_SECRET, causing loop-fail

### P3 — CPU busy% understates iowait
- `crates/nv-daemon/src/health_poller.rs:273`
- Only fields[3] used as idle, excludes iowait

### P3 — Disk used% understates reserved blocks
- `crates/nv-daemon/src/health_poller.rs:353`
- Uses f_bfree instead of f_bavail (excludes root-reserved blocks)

### P3 — ServiceInstanceConfig is empty marker struct
- Dead abstraction carried through ServiceConfig<T> generic

## Files to Modify
- `crates/nv-daemon/src/health.rs` (status degradation)
- `crates/nv-daemon/src/health_poller.rs` (hardcoded paths, CPU, disk)
- `crates/nv-cli/src/main.rs` (CLI stubs)
- `crates/nv-core/src/config.rs` (validation)
- `crates/nv-daemon/src/state.rs` (race condition)
- `crates/nv-daemon/src/claude.rs` (hardcoded path)
- `crates/nv-daemon/src/callbacks.rs` (hardcoded path)
- `deploy/nv.service` (watchdog)
- `deploy/install.sh` (Teams relay)
