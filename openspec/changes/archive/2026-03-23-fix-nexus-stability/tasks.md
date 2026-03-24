# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [api-engineer] Remove duplicate `self.consecutive_failures += 1` from the `Err` branch of `reconnect()` — `mark_disconnected()` already increments before this call — `crates/nv-daemon/src/nexus/connection.rs`
- [x] [api-engineer] Add `found` boolean flag in `send_command`; set it to `true` on `Ok(response)`; return `Ok(output)` when `found` is true regardless of whether output is empty — `crates/nv-daemon/src/nexus/client.rs`
- [x] [api-engineer] Remove `#[allow(dead_code)]` from `SessionSummary` struct declaration — `crates/nv-daemon/src/nexus/client.rs`
- [x] [api-engineer] Remove `#[allow(dead_code)]` from `SessionDetail` struct declaration — `crates/nv-daemon/src/nexus/client.rs`
- [x] [api-engineer] In `handle_reconnect_success`, capture `conn.disconnected_since` as a local `Option<Instant>` before calling `conn.reconnect()`; use the captured value to compute downtime display — `crates/nv-daemon/src/nexus/watchdog.rs`
- [x] [api-engineer] In `process_agent`, split reconnect into two phases: lock + read `consecutive_failures` + compute backoff + drop lock → sleep → re-lock + call `connect()` directly — `crates/nv-daemon/src/nexus/watchdog.rs`
- [x] [api-engineer] In `run_event_stream`, split the reconnect-on-stream-end block (line ~110) into two phases: lock + mark_disconnected + compute backoff + drop lock → sleep → re-lock + call `connect()` directly — `crates/nv-daemon/src/nexus/stream.rs`
- [x] [api-engineer] Extend the `EventFilter` in `run_event_stream` to include `EventType::SessionStarted` and `EventType::Heartbeat` so the `Started` and `Heartbeat` match arms in `map_event_to_trigger` are reachable — `crates/nv-daemon/src/nexus/stream.rs`
- [x] [api-engineer] Replace byte-index slice `&answer_text[..TELEGRAM_MAX_CHARS - 30]` with a UTF-8-safe char boundary using `char_indices` or `floor_char_boundary` before slicing — `crates/nv-daemon/src/query/format.rs`
- [x] [api-engineer] Remove `#[allow(dead_code)]` from `pub mod format` and `pub mod gather` (or whichever submodules have active callers); keep suppression only on genuinely unused modules — `crates/nv-daemon/src/query/mod.rs`

## Verify

- [x] [api-engineer] `cargo build` passes with no errors
- [x] [api-engineer] `cargo clippy -- -D warnings` passes with no warnings
- [x] [api-engineer] Unit test: single `mark_disconnected()` + failed `reconnect()` results in `consecutive_failures == 1`, not 2 — `crates/nv-daemon/src/nexus/connection.rs`
- [x] [api-engineer] Unit test: `send_command` returns `Ok("")` when the RPC succeeds but the stream yields zero text chunks (no fallthrough to next agent) — `crates/nv-daemon/src/nexus/client.rs`
- [x] [api-engineer] Unit test: `format_query_for_telegram` does not panic when the 4066-byte cut point falls inside a multi-byte UTF-8 sequence — `crates/nv-daemon/src/query/format.rs`
- [x] [api-engineer] Existing tests pass (`cargo test`)
