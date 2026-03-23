# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Update `StreamJsonInput` struct — replace flat `{"type":"user","content":"..."}` with nested `{"message":{"role":"user","content":"..."}}` format; change struct fields and Serialize derive/impl accordingly [owner:api-engineer]
- [x] [2.2] [P-1] Update `send_turn()` — remove `msg_type` field usage, construct `StreamJsonInput` with the new `message.role` + `message.content` structure [owner:api-engineer]
- [x] [2.3] [P-1] Switch HOME in `spawn_persistent()` — replace `.env("HOME", &config.sandbox_home)` with `.env("HOME", &config.real_home)` [owner:api-engineer]
- [x] [2.4] [P-1] Remove `sandbox_home` field from `SpawnConfig` struct and its initialization in `SpawnConfig::new()` [owner:api-engineer]
- [x] [2.5] [P-1] Add `drain_init_events()` async function — reads stdout lines after spawn, skips all `{"type":"system",...}` events, returns `Ok(Option<String>)` where Some holds the first non-system line (buffered for first turn) [owner:api-engineer]
- [x] [2.6] [P-1] Add 10-second timeout to `drain_init_events()` — on timeout, log warning and return Ok(None) to proceed without buffered line [owner:api-engineer]
- [x] [2.7] [P-1] Handle EOF in `drain_init_events()` — if subprocess closes stdout during drain, return error so caller falls back to cold-start [owner:api-engineer]
- [x] [2.8] [P-1] Call `drain_init_events()` in `spawn_persistent()` after capturing stdin/stdout — store any buffered first line in `PersistentProcess` struct [owner:api-engineer]
- [x] [2.9] [P-1] Add `buffered_line: Option<String>` field to `PersistentProcess` struct — consumed by the first `read_stream_response` call before reading from stdout [owner:api-engineer]
- [x] [2.10] [P-2] Add ready detection after drain — call `child.try_wait()` after drain completes; if `Some(status)` returned, process died, return error [owner:api-engineer]
- [x] [2.11] [P-2] Update `read_stream_response_from_lines` (or caller) to prepend `buffered_line` to the event stream on first read, then clear it [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: `StreamJsonInput` serializes to `{"message":{"role":"user","content":"test"}}` format [owner:api-engineer]
- [x] [3.4] Unit test: drain_init_events skips system events and returns first non-system line [owner:api-engineer]
- [x] [3.5] Unit test: drain_init_events returns Ok(None) on timeout with no non-system event [owner:api-engineer]
- [x] [3.6] Unit test: drain_init_events returns Err on EOF (subprocess died) [owner:api-engineer]
- [x] [3.7] Existing tests pass (stream response parsing, cold-start fallback) [owner:api-engineer]
