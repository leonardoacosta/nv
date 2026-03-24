# Context: Fix Watcher Reliability Issues

## Source: Audit 2026-03-23 (watchers domain, 75/B- health)

## Problem
Missing cooldown causes obligation flooding, watchers aren't cancelled on shutdown, hardcoded paths.

## Findings

### P1 — No cooldown on rule firing
- `crates/nv-daemon/src/watchers/mod.rs` — evaluate_rule
- `last_triggered_at` is written after every fire but never read before next evaluation
- Persistent external failure creates one obligation per watcher cycle (every 5 minutes)
- Floods obligation store with duplicates
- Fix: Read rule.last_triggered_at; skip if now - last_triggered < cooldown_secs

### P2 — JoinHandle discarded — watchers not cancelled on SIGTERM
- `crates/nv-daemon/src/main.rs:723` — spawn_watchers() return value discarded
- In-flight watcher tasks abandoned on daemon stop
- Fix: Bind handle; handle.abort() on shutdown select arm

### P2 — Hardcoded /home/nyaptor fallback in obligation detector
- `crates/nv-daemon/src/obligation_detector.rs:134`
- When HOME env var unavailable, falls back to hardcoded path
- Fix: Return Err when HOME unavailable; remove hardcoded path

### P2 — DeployWatcher debug-level when projects key absent
- `crates/nv-daemon/src/watchers/deploy_watcher.rs:56`
- Empty projects list logged at debug level — should be warn with config hint

### P3 — Sentry issue.count parse failure silent
- `crates/nv-daemon/src/watchers/sentry_watcher.rs:84`
- String parse failure uses unwrap_or(false), no log
- Fix: Add tracing::debug! on parse error

### P3 — ObligationDetector has no subprocess timeout
- `crates/nv-daemon/src/obligation_detector.rs:162`
- wait_with_output has no timeout wrapper
- Fix: Wrap in tokio::time::timeout(Duration::from_secs(30), ...)

### P3 — HA watcher N+1 sequential entity HTTP calls
- `crates/nv-daemon/src/watchers/ha_watcher.rs:80`
- Sequential HTTP call per entity
- Fix: Use /api/states bulk endpoint or tokio::join! in parallel

### P4 — drain_with_timeout dead code
- `crates/nv-daemon/src/shutdown.rs:37` — #[allow(dead_code)]
- Not wired into shutdown path

## Files to Modify
- `crates/nv-daemon/src/watchers/mod.rs` (cooldown logic)
- `crates/nv-daemon/src/watchers/deploy_watcher.rs` (log level)
- `crates/nv-daemon/src/watchers/sentry_watcher.rs` (parse logging)
- `crates/nv-daemon/src/watchers/ha_watcher.rs` (N+1 fix)
- `crates/nv-daemon/src/obligation_detector.rs` (hardcoded path, timeout)
- `crates/nv-daemon/src/main.rs` (JoinHandle, shutdown)
- `crates/nv-daemon/src/shutdown.rs` (wire drain_with_timeout)
