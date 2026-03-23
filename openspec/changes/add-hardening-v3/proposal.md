# Proposal: Hardening v3

## Change ID
`add-hardening-v3`

## Summary

Three targeted bug fixes for the Nova daemon: validate Jira project KEYs before API calls,
deduplicate consecutive PendingAction error messages in Telegram, and enforce a configurable
per-worker timeout so hung workers cannot consume slots indefinitely.

## Context
- Extends: `crates/nv-daemon/src/jira/tools.rs` (tool input handling), `crates/nv-daemon/src/jira/client.rs` (API calls, optional project cache), `crates/nv-daemon/src/orchestrator.rs` (PendingAction error delivery), `crates/nv-daemon/src/worker.rs` (Worker::run, dispatch, tokio::spawn), `crates/nv-core/src/config.rs` (DaemonConfig)
- Related: `jira_create` tool definition already documents "project KEY (uppercase, 2-4 chars)" in its description, but the handler does not enforce this at runtime. Workers already have per-tool timeouts (`TOOL_TIMEOUT_READ`, `TOOL_TIMEOUT_WRITE`) but no overall session-level timeout.
- Depends on: nothing — standalone hardening

## Motivation

**Jira KEY validation:** When Claude passes an invalid project string (e.g., a full project name
instead of the 2-10 char uppercase KEY), the Jira API returns a 400 "valid project is required"
error. This wastes a tool-loop iteration and produces a confusing error for the user. Catching
this before the HTTP call gives an immediate, actionable error.

**Error deduplication:** When multiple PendingActions fail with the same root cause (e.g., a Nexus
session crashes and 4 queued actions all fail), each failure sends a separate Telegram message.
This spams the user with identical error text. Batching consecutive failures with the same error
into a single message reduces noise.

**Worker timeout:** If a Claude API call hangs or a tool enters an infinite loop, the worker holds
its slot forever. With the default `max_workers: 3`, a single hung worker degrades capacity by
33%. A hard timeout ensures slots are always reclaimed.

## Requirements

### Req-1: Jira Project KEY Validation

Add input validation to the `jira_create` tool handler in `tools.rs` before the Jira API call.

- Validate the `project` field matches `^[A-Z][A-Z0-9]{1,9}$` (2-10 uppercase alphanumeric, starts
  with a letter) — this matches Jira's actual KEY constraints
- Return an immediate tool error if validation fails: `"Invalid project KEY '{value}'. Must be 2-10 uppercase letters/digits starting with a letter (e.g., OO, TC, MV)."`
- Optionally: add a `project_keys_cache` to `JiraClient` that fetches and caches the list of valid
  project keys on first use (GET `/rest/api/3/project`), and cross-check the KEY against the cache.
  Cache TTL: 1 hour. This is a nice-to-have — the regex validation alone is the primary fix.

### Req-2: PendingAction Error Deduplication

When sending PendingAction failure messages to Telegram, batch consecutive errors with identical
error text within a 2-second debounce window.

- Track a `(last_error_text, last_error_time, error_count)` tuple in the orchestrator
- When a PendingAction failure is about to send a Telegram message:
  - If `error_text == last_error_text` AND `now - last_error_time < 2s`: increment `error_count`,
    reset the timer, do NOT send yet
  - Otherwise: flush the previous batch (if any) as a single message, start a new batch
- Flush format: `"{count} actions failed: {error_text}"` (or just the error text if count == 1)
- On flush (timer expiry or new different error): send the batched message
- Implementation: use a `tokio::time::sleep` future that fires 2s after the last error in a batch,
  triggering the flush. Cancel and restart on each new matching error.

### Req-3: Worker-Level Timeout

Wrap the `Worker::run()` call in the dispatch spawned task with `tokio::time::timeout`.

- Default timeout: 300 seconds (5 minutes)
- Configurable via `nv.toml`: `[daemon] worker_timeout_secs = 300`
- Add `worker_timeout_secs` field to `DaemonConfig` with `#[serde(default = "default_worker_timeout_secs")]` and a default function returning `300`
- In the dispatch `tokio::spawn` block, wrap `Worker::run(...)` with `tokio::time::timeout(duration, Worker::run(...))`
- On timeout:
  - Emit `WorkerEvent::Error { worker_id, error: "Worker timed out after {N}s" }`
  - Decrement the active worker count (return the slot)
  - Send a Telegram error message to the originating chat: `"Request timed out after {N}s. Try again or simplify the request."`
  - Log at `warn` level with the worker_id and task details
- The existing per-tool timeouts remain unchanged — the worker timeout is a hard ceiling above them

## Scope
- **IN**: Jira KEY regex validation in tool handler, optional project key cache in JiraClient, PendingAction error dedup with 2s debounce in orchestrator, worker-level timeout wrapper in dispatch, `worker_timeout_secs` config field
- **OUT**: retry logic for timed-out workers (user retries manually), Jira KEY validation for `jira_transition`/`jira_assign`/`jira_comment` (those take issue keys, not project keys), per-tool timeout changes, UI for pending error batches beyond the single message

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/jira/tools.rs` | Add KEY regex validation before `jira_create` API call |
| `crates/nv-daemon/src/jira/client.rs` | Optional: add `get_projects()` method + in-memory cache for project key validation |
| `crates/nv-daemon/src/orchestrator.rs` | Add error dedup state (`last_error_text`, `last_error_time`, `error_count`), debounce flush logic for PendingAction failures |
| `crates/nv-daemon/src/worker.rs` | Wrap `Worker::run()` in `tokio::time::timeout` in dispatch, handle timeout variant |
| `crates/nv-core/src/config.rs` | Add `worker_timeout_secs: u64` to `DaemonConfig` with default 300 |

## Risks
| Risk | Mitigation |
|------|-----------|
| KEY regex rejects valid Jira keys outside `[A-Z][A-Z0-9]{1,9}$` | Jira docs confirm keys are uppercase, start with letter, 2-10 chars. The regex matches the official constraint. If an edge case arises, the optional cache lookup serves as a fallback. |
| Error dedup 2s window loses individual error context | Each error is still logged at `warn` level with full details. Only the Telegram user-facing message is batched. Count is included so the user knows how many failed. |
| Worker timeout kills a legitimately long-running task | 5 minutes is generous — the longest normal tool chain completes in ~90s. The timeout is configurable via `nv.toml` for users who need longer. A `warn` log captures which task timed out for debugging. |
| `tokio::time::timeout` cancellation leaves tool side-effects partially applied | Tools that make external API calls (Jira create, etc.) may have already committed. This is acceptable — the user sees the timeout error and can check state. Same behavior as a process crash. |
| Project key cache becomes stale if a new project is created | 1-hour TTL ensures the cache refreshes. For immediate needs, the regex validation still passes valid-format keys through to the API, which returns the authoritative error. |
