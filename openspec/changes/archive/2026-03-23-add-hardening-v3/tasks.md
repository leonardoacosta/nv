# Implementation Tasks

<!-- beads:epic:TBD -->

## Jira Project KEY Validation

- [x] [1.1] [P-1] Add KEY regex validation to `jira_create` handler in `tools.rs` — validate `project` field matches `^[A-Z][A-Z0-9]{1,9}$` before API call, return descriptive tool error on mismatch [owner:api-engineer]
- [x] [1.2] [P-2] Add `get_projects()` method to `JiraClient` in `client.rs` — GET `/rest/api/3/project`, parse response into `Vec<String>` of project keys [owner:api-engineer]
- [x] [1.3] [P-2] Add in-memory project key cache to `JiraClient` with 1-hour TTL — `Option<(Vec<String>, Instant)>` field, refresh on cache miss or expiry [owner:api-engineer]
- [x] [1.4] [P-2] Cross-check validated KEY against project cache in `jira_create` handler — warn if KEY format is valid but not in cache (soft warning, do not block the call) [owner:api-engineer]
- [x] [1.5] [P-2] Add tests: valid KEYs pass regex, invalid KEYs rejected (lowercase, too short, too long, starts with digit, special chars) [owner:api-engineer]

## PendingAction Error Deduplication

- [x] [2.1] [P-1] Add error dedup state to orchestrator — `last_error_text: Option<String>`, `last_error_time: Instant`, `error_count: u32`, and a `tokio::sync::Notify` (or `sleep` future) for flush [owner:api-engineer]
- [x] [2.2] [P-1] Implement debounce logic in PendingAction failure path — on matching error within 2s, increment count and reset timer; on different error or first error, flush previous batch and start new batch [owner:api-engineer]
- [x] [2.3] [P-1] Implement flush: send batched message to Telegram — format as `"{count} actions failed: {error}"` when count > 1, plain error when count == 1 [owner:api-engineer]
- [x] [2.4] [P-2] Add timer-based flush — spawn a 2s delayed task that flushes the current batch if no new matching errors arrive [owner:api-engineer]
- [x] [2.5] [P-2] Add tests: single error sends immediately after 2s, two identical errors within 2s batch into one message, different errors flush previous batch [owner:api-engineer]

## Worker-Level Timeout

- [x] [3.1] [P-1] Add `worker_timeout_secs: u64` field to `DaemonConfig` in `config.rs` with `#[serde(default = "default_worker_timeout_secs")]` defaulting to 300 [owner:api-engineer]
- [x] [3.2] [P-1] Wrap `Worker::run()` call in dispatch `tokio::spawn` with `tokio::time::timeout(Duration::from_secs(timeout), Worker::run(...))` [owner:api-engineer]
- [x] [3.3] [P-1] Handle timeout variant — emit `WorkerEvent::Error` with timeout message, decrement active worker count, send Telegram error to originating chat [owner:api-engineer]
- [x] [3.4] [P-2] Add `warn`-level tracing log on timeout with worker_id, task_id, and trigger summary [owner:api-engineer]
- [x] [3.5] [P-2] Add test: verify timeout config deserialization with default and explicit values [owner:api-engineer]

## Verify

- [x] [4.1] `cargo build` passes [owner:api-engineer]
- [x] [4.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [4.3] `cargo test` — existing tests pass, new tests for KEY validation and config deserialization [owner:api-engineer]
