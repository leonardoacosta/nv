# Nexus Domain Audit Memory

**Audited:** 2026-03-23  
**Scope:** `crates/nv-daemon/src/nexus/` + `crates/nv-daemon/src/query/`  
**Files reviewed:** client.rs, connection.rs, stream.rs, tools.rs, progress.rs, notify.rs, watchdog.rs, query/{gather,synthesize,format,followup,mod}.rs, proto/nexus.proto

---

## Checklist Results

| Area | Item | Result |
|------|------|--------|
| Client | Multi-agent connect logic | PASS — parallel connect_all, partial connectivity tolerated |
| Client | Session filter queries | PASS — all filters supported, sorted newest-first |
| Client | Thread safety (Arc<Mutex>) | PASS — per-agent locking, no cross-agent lock contention |
| Client | Connection pooling and reuse | CONCERN — no pooling; one gRPC channel per agent |
| Connection | gRPC channel lifecycle | PASS — connect/reconnect/mark_disconnected all implemented |
| Connection | ConnectionStatus transitions | PASS — Connected/Disconnected/Reconnecting with proper guards |
| Connection | Timeout on gRPC calls | PASS — CONNECT_TIMEOUT (10s) on connect, 5s on health_check |
| Connection | TLS/auth configuration | CONCERN — plaintext http:// hardcoded; no TLS path available |
| Connection | Backoff algorithm | PASS — exponential 1s..60s, capped at failures.min(6) |
| Connection | Quarantine after 10 failures | PASS — 5-minute quarantine, is_quarantined guard in watchdog |
| Stream | Event stream reconnect on drop | PASS — outer loop reconnects on stream end or error |
| Stream | Backpressure handling | PASS — unbounded mpsc, server controls stream pace |
| Stream | Event filter alignment | FAIL — filter includes STATUS_CHANGED + SESSION_STOPPED but handler has Started/Heartbeat arms that are unreachable |
| Stream | Mutex held during reconnect sleep | FAIL — lock held across conn.reconnect().await (up to 60s sleep) |
| Watchdog | Heartbeat interval | PASS — configurable watchdog_interval_secs, stale threshold = 3× interval |
| Watchdog | Reconnection strategy | PASS — immediate reconnect on health check failure, quarantine at 10 failures |
| Watchdog | Health status aggregation | PASS — per-agent ChannelStatus updates to HealthState |
| Watchdog | Mutex held during reconnect | FAIL — same pattern as stream; process_agent holds lock during full backoff |
| Notifications | Session start/complete/error | PASS — Completed and Failed routed; Started/Progress suppressed |
| Notifications | Routing (Telegram) | PASS — channel map lookup with graceful fallback if not registered |
| Notifications | Deduplication | PASS — disconnect_notified flag per connection, 30s debounce |
| Notifications | Reconnect downtime display | FAIL — always shows "unknown"; disconnected_since cleared by connect() before capture |
| Query | gather.rs — parallel data collection | PASS — tokio::join! with 15s timeouts per source |
| Query | synthesize.rs — multi-source context | PASS — XML-tagged sections, string-filter guards for empty/error results |
| Query | format.rs — display formatting | CONCERN — byte-slice truncation can panic on multi-byte UTF-8 at 4066-byte boundary |
| Query | followup.rs — TTL and persistence | PASS — 5-minute TTL, auto-cleanup on expiry |

---

## Bugs (must fix)

### BUG-1: Double failure counter increment (MEDIUM)
**File:** `crates/nv-daemon/src/nexus/connection.rs` lines 90 + 187  
`mark_disconnected()` increments `consecutive_failures`, then `reconnect()` increments it again on failure. A single failed reconnect inflates the count by 2. With quarantine threshold at 10, an agent quarantines after ~5 actual attempts instead of 10.

**Fix:** Remove the `self.consecutive_failures += 1` from `reconnect()` (line 187). `mark_disconnected()` is always called before `reconnect()` so the count is already incremented.

### BUG-2: `send_command` silent false-negative (MEDIUM)
**File:** `crates/nv-daemon/src/nexus/client.rs` lines 364-369  
When a gRPC `SendCommand` call returns successfully but the stream produces no text chunks (agent ran command but had no output), the code falls through to the next agent. The session exists and ran; this is a valid empty response, not a "not found" condition.

**Fix:** Track whether we successfully received a response from the right agent (e.g., with a `found_session: bool` flag) and return `Ok(output)` even when `output.is_empty()` if the stream completed normally on this agent.

### BUG-3: Reconnect notification shows "unknown" downtime (MEDIUM)
**File:** `crates/nv-daemon/src/nexus/watchdog.rs` line 227  
`handle_reconnect_success` acknowledges the problem in a comment but never solves it. `disconnected_since` is cleared by `connect()` before the downtime is computed.

**Fix:** Capture `conn.disconnected_since` immediately before calling `conn.reconnect()` in both branches of `process_agent`. Pass the captured instant into `handle_reconnect_success` and compute the elapsed duration from it.

---

## Risks (assess before next release)

### RISK-1: Mutex held across full reconnect backoff (LOW-MEDIUM)
**Files:** `watchdog.rs` line 71, `stream.rs` line 110  
Both the watchdog and event stream hold the per-agent `Arc<Mutex<NexusAgentConnection>>` while sleeping inside `reconnect().await` (sleep up to 60s). Any concurrent operation on the same agent (HTTP handler, tool call) blocks for the full backoff duration.

**Recommendation:** Release the lock before sleeping. Redesign `reconnect()` to be split into `begin_reconnect()` (mark as Reconnecting, return the backoff duration) and `attempt_connect()` (the actual gRPC call), so the caller can release the lock, sleep, and re-acquire.

### RISK-2: No TLS on gRPC channel (LOW)
**File:** `crates/nv-daemon/src/nexus/connection.rs` line 53  
Endpoint is always `http://`. Acceptable for LAN, but blocks secure remote deployments and provides no defense against a compromised router on the local network.

**Recommendation:** Add optional TLS support via tonic's `ClientTlsConfig`. Read a config flag; if TLS cert/domain is provided, use `Channel::from_shared(...).tls_config(...)`.

### RISK-3: Unreachable match arms in event stream (LOW)
**File:** `crates/nv-daemon/src/nexus/stream.rs` lines 164–183  
`Started` and `Heartbeat` payload arms exist in `map_event_to_trigger` but the `EventFilter` never requests those event types. This is either dead code or the filter is missing entries.

---

## Debt-inducing issues (schedule cleanup)

| # | File | Issue |
|---|------|-------|
| D1 | `client.rs:13,27` | `#[allow(dead_code)]` on SessionSummary/SessionDetail — all fields are used; attribute masks future dead fields |
| D2 | `query/mod.rs:1-8` | All four submodules suppressed with `#[allow(dead_code)]` — three submodules (format, gather, synthesize) appear unused outside the module |
| D3 | `progress.rs:110` | `/ci` substring check is broader than `/ci:gh` intent |
| D4 | `query/format.rs:14` | Byte-index truncation of potentially multi-byte UTF-8 strings can panic |
| D5 | `query/gather.rs:96` | Memory::search runs on async executor without spawn_blocking |
| D6 | `query/gather.rs:177` | Multi-project JQL extraction silently drops all but the first project key |

---

## Architecture observations

- **Good:** Partial connectivity model is well-designed. All query methods tolerate disconnected agents gracefully with warnings, not panics.
- **Good:** Event stream respawn logic in watchdog is correct — `is_finished()` check with index-based handle replacement covers the main lifecycle.
- **Good:** Quarantine prevents thundering-herd reconnect storms after sustained outages.
- **Good:** Query domain uses parallel `tokio::join!` with independent timeouts — resilient to slow data sources.
- **Concern:** The `#[allow(clippy::too_many_arguments)]` on `handle_reconnect_success` (7 params) is a smell. The watchdog function should bundle reconnect context into a typed struct.
