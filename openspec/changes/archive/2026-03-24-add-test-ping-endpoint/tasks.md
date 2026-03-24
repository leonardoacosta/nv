# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [1.1] [P-1] Add `trigger_tx: mpsc::UnboundedSender<Trigger>` to the HTTP server state so routes can inject triggers -- already present in `HttpState` [owner:api-engineer]
- [x] [1.2] [P-1] Reused existing `CliCommand::Ask` + oneshot pattern from `/ask` handler -- no new worker event subscription needed [owner:api-engineer]
- [x] [1.3] [P-2] Implement `GET /test/ping` route handler: inject `CliCommand::Ask("ping")`, await response via oneshot, return JSON `{ok, elapsed_ms, response_preview}` with 60s timeout and AtomicBool concurrency guard -- `crates/nv-daemon/src/http.rs` [owner:api-engineer]

## Verify

- [x] [2.1] Integration test: `curl http://127.0.0.1:8400/test/ping` returns `{"ok": true, "elapsed_ms": 5354}` -- 5.3s response [owner:api-engineer]
- [x] [2.2] Concurrency test: second concurrent ping rejected while first in progress [owner:api-engineer]
- [x] [2.3] Existing tests pass: `cargo test -p nv-daemon http` -- 18 passed [owner:api-engineer]
