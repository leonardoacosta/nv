# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Fix `parse_tool_calls()` to loop over all ` ```tool_call ` blocks instead of returning after the first match — collect all `ContentBlock::ToolUse` entries before returning — `crates/nv-daemon/src/claude.rs:1183` [owner:api-engineer]
- [x] [1.2] [P-1] Preserve interleaved text blocks (text before each tool call) in the multi-call loop — `crates/nv-daemon/src/claude.rs:1183` [owner:api-engineer]
- [x] [1.3] [P-1] Fix worker queue dequeue race — restructure dequeue in `WorkerPool::spawn()` so `active.fetch_add(1)` for the next task occurs inside the queue lock scope, eliminating the window between fetch_sub and fetch_add — `crates/nv-daemon/src/worker.rs:354` [owner:api-engineer]
- [x] [1.4] [P-1] Add queued-worker timeout parity — emit `WorkerEvent::Error` via `deps.event_tx` in the queued-worker `Err(_elapsed)` branch — `crates/nv-daemon/src/worker.rs:386` [owner:api-engineer]
- [x] [1.5] [P-1] Add queued-worker timeout Telegram notification — send `OutboundMessage` to the originating chat in the queued-worker timeout branch, mirroring the primary worker timeout pattern at lines 329-348 — `crates/nv-daemon/src/worker.rs:386` [owner:api-engineer]
- [x] [1.6] [P-2] Add `health_port: u16` field to `SharedDeps` struct — `crates/nv-daemon/src/worker.rs:159` [owner:api-engineer]
- [x] [1.7] [P-2] Populate `SharedDeps.health_port` from `config.daemon.health_port` (default 8400) in `main.rs` — `crates/nv-daemon/src/main.rs:1004` [owner:api-engineer]
- [x] [1.8] [P-2] Fix `cmd_digest()` to read port from `self.deps.health_port` instead of hardcoded `8400` — `crates/nv-daemon/src/orchestrator.rs:1009` [owner:api-engineer]
- [x] [1.9] [P-2] Fix `is_quiet_hours()` to accept a `tz_name: &str` parameter and use `tz_offset_seconds()` from `reminders.rs` to compute the user's local `NaiveTime` from UTC, replacing `chrono::Local::now().time()` — `crates/nv-daemon/src/orchestrator.rs:1775` [owner:api-engineer]
- [x] [1.10] [P-2] Update all `is_quiet_hours()` call sites in `orchestrator.rs` to pass `&self.deps.timezone` as the new argument — `crates/nv-daemon/src/orchestrator.rs:394,424` [owner:api-engineer]
- [x] [1.11] [P-2] Add `worker_stage_pending_removal: HashSet<Uuid>` field to `Orchestrator` — `crates/nv-daemon/src/orchestrator.rs:190` [owner:api-engineer]
- [x] [1.12] [P-2] Change `WorkerEvent::StageComplete` handler to insert worker_id into `worker_stage_pending_removal` instead of immediately removing from `worker_stage_started` — `crates/nv-daemon/src/orchestrator.rs:660` [owner:api-engineer]
- [x] [1.13] [P-2] Change `WorkerEvent::ToolCalled` handler to remove worker_id from `worker_stage_pending_removal` (cancelling deferred removal) — `crates/nv-daemon/src/orchestrator.rs` [owner:api-engineer]
- [x] [1.14] [P-2] In `check_inactivity()`, flush `worker_stage_pending_removal` — remove any workers in the pending set from `worker_stage_started` — `crates/nv-daemon/src/orchestrator.rs` [owner:api-engineer]

## Verify

- [x] [2.1] `cargo build` passes [owner:api-engineer]
- [x] [2.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [2.3] Unit test: `parse_tool_calls()` with two tool_call blocks returns two `ContentBlock::ToolUse` entries — `crates/nv-daemon/src/claude.rs` [owner:api-engineer]
- [x] [2.4] Unit test: `parse_tool_calls()` with one tool_call block followed by plain text returns one `ToolUse` and one `Text` block [owner:api-engineer]
- [x] [2.5] Unit test: `is_quiet_hours()` with explicit `tz_name` matches expected in-window result for a UTC-offset timezone [owner:api-engineer]
- [x] [2.6] Existing tests pass [owner:api-engineer]
