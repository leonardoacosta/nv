# Context: Fix Nexus Stability Issues

## Source: Audit 2026-03-23 (nexus domain, 84/B health — healthiest domain)

## Problem
Double failure counter accelerates quarantine, send_command false-negative on empty output, mutex held during backoff sleep.

## Findings

### P1 — Double failure counter increment
- `crates/nv-daemon/src/nexus/connection.rs:90` — mark_disconnected() increments consecutive_failures
- `crates/nv-daemon/src/nexus/connection.rs:187` — reconnect() Err branch increments again
- Single failed reconnect counts as 2 failures
- Quarantine threshold at 10 → agent quarantines after ~5 real attempts
- Fix: Remove the += 1 from reconnect() Err branch

### P1 — send_command silent false-negative on empty output
- `crates/nv-daemon/src/nexus/client.rs` — send_command
- When RPC succeeds but response stream produces zero text chunks (valid empty output)
- Code falls through to try next agent
- Final error "Session not found on any connected agent" is misleading
- Fix: Track found flag; return Ok(output) regardless of whether output is empty

### P2 — Reconnect notification shows "unknown" downtime
- `crates/nv-daemon/src/nexus/watchdog.rs` — handle_reconnect_success
- disconnected_since cleared by connect() before downtime computed
- Fix: Capture disconnected_since as local var before calling reconnect()

### P2 — Mutex held across full reconnect backoff sleep (up to 60s)
- `crates/nv-daemon/src/nexus/watchdog.rs:71` (process_agent)
- `crates/nv-daemon/src/nexus/stream.rs:110` (run_event_stream)
- Arc<Mutex<NexusAgentConnection>> held while sleeping in reconnect()
- Any concurrent operation on same agent blocks for entire backoff
- Fix: Split reconnect into two phases — compute backoff outside lock, sleep, re-acquire, connect

### P3 — Unreachable match arms in stream event handler
- `crates/nv-daemon/src/nexus/stream.rs` — map_event_to_trigger
- Handles Started and Heartbeat variants, but EventFilter only subscribes to STATUS_CHANGED and SESSION_STOPPED
- Either filter is incomplete or handler arms are dead code

### P3 — Byte-index truncation in format_query_for_telegram
- `crates/nv-daemon/src/query/format.rs` — 4066-byte cut point
- Same UTF-8 panic pattern as Telegram/digest (covered in fix-channel-safety)

### P4 — #[allow(dead_code)] on fully-used public structs
- SessionSummary, SessionDetail — all fields used, attribute is noise

### P4 — query/mod.rs blanket #[allow(dead_code)] on all four submodules
- Hides unused modules; remove for modules with callers

## Files to Modify
- `crates/nv-daemon/src/nexus/connection.rs` (failure counter)
- `crates/nv-daemon/src/nexus/client.rs` (send_command)
- `crates/nv-daemon/src/nexus/watchdog.rs` (downtime, mutex split)
- `crates/nv-daemon/src/nexus/stream.rs` (unreachable arms, mutex)
- `crates/nv-daemon/src/query/format.rs` (UTF-8 truncation)
- `crates/nv-daemon/src/query/mod.rs` (dead_code attrs)
