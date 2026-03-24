# Proposal: Fix Nexus Stability

## Change ID
`fix-nexus-stability`

## Summary

Eight correctness and hygiene fixes across the Nexus subsystem: eliminate a double failure-counter
increment that halves the effective quarantine threshold, fix a false-negative in `send_command`
that silently discards valid empty responses, compute reconnect downtime before `connect()` clears
it, split mutex-held reconnect backoff sleeps into two phases, align the stream event filter with
the handler arms it actually serves, fix a byte-index UTF-8 truncation in the query formatter,
and remove stale `#[allow(dead_code)]` attributes.

## Context
- Extends: `crates/nv-daemon/src/nexus/connection.rs` (failure counter, reconnect)
- Extends: `crates/nv-daemon/src/nexus/client.rs` (send_command, SessionSummary/SessionDetail)
- Extends: `crates/nv-daemon/src/nexus/watchdog.rs` (downtime display, mutex split)
- Extends: `crates/nv-daemon/src/nexus/stream.rs` (event filter, mutex split)
- Extends: `crates/nv-daemon/src/query/format.rs` (UTF-8 truncation)
- Extends: `crates/nv-daemon/src/query/mod.rs` (dead_code suppression)
- Related: Audit 2026-03-23 (nexus domain, 84/B health)

## Motivation

The Nexus domain scored 84/B in the March 2026 audit — the healthiest domain — but carries eight
concrete bugs ranging from correctness (P1) to hygiene (P4). Two P1 bugs affect production
reliability:

1. **Double failure counter** — `mark_disconnected()` increments `consecutive_failures` and then
   `reconnect()` increments it again on `Err`. A single failed reconnect attempt counts as two
   failures, so the quarantine threshold of 10 is effectively 5 real attempts. Agents that hit a
   transient blip get quarantined twice as fast as intended.

2. **send_command false-negative** — after a successful RPC the stream is consumed, but if the
   response produces zero text chunks (valid for commands with no text output) the code falls
   through to try the next agent, eventually returning "Session not found on any connected agent".
   Commands with empty output silently fail.

The P2 bugs degrade operator experience: the reconnect Telegram notification always shows
"unknown" downtime because `connect()` clears `disconnected_since` before the notification is
composed, and holding the `Arc<Mutex<NexusAgentConnection>>` lock across the full exponential
backoff sleep (up to 60 s) blocks every concurrent operation on that agent.

The P3/P4 items are hygiene: unreachable match arms that suggest filter/handler misalignment,
a UTF-8 byte-boundary panic risk in the query formatter, and suppressed dead-code warnings that
hide real signal.

## Requirements

### Req-1: Remove duplicate failure increment in reconnect()

In `connection.rs`, the `Err` branch of `reconnect()` must not increment `consecutive_failures`.
`mark_disconnected()` already incremented it before `reconnect()` was called. One failed attempt
= one increment.

### Req-2: Fix send_command false-negative on empty output

In `client.rs`, `send_command` must track whether it found the session on a given agent
independently of whether output was accumulated. Introduce a `found` flag set to `true` when the
RPC call succeeds (i.e., `Ok(response)`). Return `Ok(output)` — even if `output.is_empty()` —
when `found` is true. Only continue to the next agent on a genuine `NotFound` status code.

### Req-3: Capture disconnected_since before connect() clears it

In `watchdog.rs`, `handle_reconnect_success` must read `conn.disconnected_since` before calling
`conn.reconnect()` (which internally calls `connect()`, which clears `disconnected_since`).
Capture it as a local `Option<Instant>`, then compute the downtime display from that captured
value. If the captured value is `None`, display "unknown".

### Req-4: Split mutex-held reconnect into two phases

Both `watchdog.rs` (process_agent) and `stream.rs` (run_event_stream) hold the
`Arc<Mutex<NexusAgentConnection>>` guard while `reconnect()` sleeps for up to 60 s.

Fix pattern (both sites):
1. Lock, read `consecutive_failures`, compute `backoff_duration()`, drop lock.
2. Sleep the backoff outside the lock.
3. Re-acquire lock, call `connect()` directly.

`reconnect()` itself can remain for other callers, but the watchdog and stream must use the
split pattern.

### Req-5: Align event filter with handler arms (or remove dead arms)

`stream.rs` subscribes to `STATUS_CHANGED` and `SESSION_STOPPED` only, but `map_event_to_trigger`
handles `Started` and `Heartbeat` as well. The filter must be extended to include
`EventType::SessionStarted` and `EventType::Heartbeat`, or the `Started` and `Heartbeat` arms
must be removed from the match. Preferred fix: add both types to the `EventFilter` so the handler
arms are reachable and the event stream is complete.

### Req-6: Fix UTF-8 byte truncation in format_query_for_telegram

`format.rs` slices `answer_text` at a byte offset (`&answer_text[..TELEGRAM_MAX_CHARS - 30]`).
If that offset falls inside a multi-byte UTF-8 character the slice will panic. Use
`answer_text.floor_char_boundary(TELEGRAM_MAX_CHARS - 30)` (stable in Rust 1.79+) or iterate
`char_indices` to find a safe cut point. The existing `rfind('\n')` approach is correct but must
operate on a safely-bounded slice.

### Req-7: Remove #[allow(dead_code)] from SessionSummary and SessionDetail

Both structs in `client.rs` have all fields actively used. The attribute is noise. Remove it from
both struct declarations.

### Req-8: Remove blanket #[allow(dead_code)] from query/mod.rs submodules

`query/mod.rs` suppresses dead_code on all four submodules (`followup`, `format`, `gather`,
`synthesize`). Suppress only modules that are genuinely unused, not all four. Remove the attribute
from any module that has callers in the codebase.

## Scope
- **IN**: failure counter fix, send_command fix, downtime capture, mutex split, event filter
  alignment, UTF-8 safe truncation, dead_code cleanup
- **OUT**: changing the quarantine threshold value, adding new event trigger types, streaming
  response changes, query/synthesize business logic

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/nexus/connection.rs` | Remove `self.consecutive_failures += 1` from `reconnect()` Err branch |
| `crates/nv-daemon/src/nexus/client.rs` | Add `found` flag in `send_command`; remove `#[allow(dead_code)]` from two structs |
| `crates/nv-daemon/src/nexus/watchdog.rs` | Capture `disconnected_since` before reconnect; split mutex across backoff sleep |
| `crates/nv-daemon/src/nexus/stream.rs` | Extend `EventFilter` with `SessionStarted`/`Heartbeat`; split mutex across backoff sleep |
| `crates/nv-daemon/src/query/format.rs` | Replace byte-index slice with UTF-8-safe char boundary |
| `crates/nv-daemon/src/query/mod.rs` | Remove `#[allow(dead_code)]` from modules with callers |

## Risks
| Risk | Mitigation |
|------|-----------|
| Removing duplicate increment changes quarantine timing — may expose slower-quarantining edge cases | The original 10-attempt threshold is the intended design; halving it was the bug |
| Mutex split in watchdog/stream leaves a window where another task could observe `Reconnecting` status during sleep | Status is set to `Reconnecting` before lock is dropped; no behavioral regression |
| Extending EventFilter to include Started/Heartbeat increases stream traffic | Handler arms for both return `None` (no trigger); no downstream impact |
| UTF-8 boundary change in format.rs modifies truncation point by up to 3 bytes | Acceptable; Telegram's 4096-char limit is advisory, not exact |
