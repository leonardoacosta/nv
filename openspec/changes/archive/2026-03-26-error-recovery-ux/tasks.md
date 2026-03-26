# Implementation Tasks

<!-- beads:epic:nv-7r14 -->

## API Batch

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/error_recovery.rs` — define `NovaError` enum with variants: `RateLimit { retry_after_secs: Option<u64> }`, `ApiOverloaded`, `Timeout`, `AuthFailure`, `ProcessCrash`, `Unknown { message: String }` [owner:api-engineer]
- [x] [2.2] [P-1] Implement `classify_error(e: &anyhow::Error) -> NovaError` in `error_recovery.rs` — map existing substring patterns (Broken pipe, EOF, process died, Timeout, timed out, hit your limit, rate limit, 429, 529, overloaded, Not logged in, auth) to variants [owner:api-engineer]
- [x] [2.3] [P-1] Implement `user_message(error: &NovaError, attempt: u32, max_attempts: u32) -> String` in `error_recovery.rs` — return per-class, per-phase (retry vs final) human-readable strings without emojis or raw error text [owner:api-engineer]
- [x] [2.4] [P-1] Implement `retry_keyboard(task_slug: &str) -> InlineKeyboard` in `error_recovery.rs` — returns single-button keyboard with `callback_data: format!("retry:{task_slug}")` [owner:api-engineer]
- [x] [2.5] [P-1] Register `pub mod error_recovery;` in `crates/nv-daemon/src/lib.rs` [owner:api-engineer]
- [x] [2.6] [P-1] Replace the error branch in `run_worker` (`worker.rs`, ~line 1230) with a retry loop: `MAX_WORKER_RETRIES = 2`, backoff `[2s, 5s]`, retryable classes = `RateLimit | ApiOverloaded | Timeout | ProcessCrash`, non-retryable = `AuthFailure | Unknown` [owner:api-engineer]
- [x] [2.7] [P-1] On each retryable attempt before final: log `tracing::warn!` with structured fields (`error_class`, `attempt`, `worker_id`), send per-attempt message to Telegram (no keyboard), sleep backoff, re-invoke `call_claude_with_fallback` [owner:api-engineer]
- [x] [2.8] [P-1] On final failure (all retries exhausted or non-retryable): emit `WorkerEvent::Error`, set red-X reaction, send final user message to Telegram with `retry_keyboard` attached (skip Retry button for `AuthFailure`) [owner:api-engineer]
- [x] [2.9] [P-1] Replace unstructured `tracing::error!(worker_task = %task_id, error = %e, ...)` with structured fields: `worker_id`, `error_class`, `attempt`, `raw_error` [owner:api-engineer]
- [x] [2.10] [P-2] Add `retry:` prefix routing in `callbacks.rs` — when callback data matches `retry:{slug}`, reconstruct a `Trigger::Message` from the slug and dispatch as a new `WorkerTask` at `Priority::High` [owner:api-engineer]
- [x] [2.11] [P-2] Add `"retry:"` to `callback_label()` in `telegram/mod.rs` — return `"Retrying..."` toast label [owner:api-engineer]

## Verify

- [x] [3.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [x] [3.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [3.3] Unit test: `classify_error` returns `RateLimit` for "hit your limit" and "rate limit" strings [owner:api-engineer]
- [x] [3.4] Unit test: `classify_error` returns `ApiOverloaded` for "529" and "overloaded" strings [owner:api-engineer]
- [x] [3.5] Unit test: `classify_error` returns `Timeout` for "Timeout" and "timed out" strings [owner:api-engineer]
- [x] [3.6] Unit test: `classify_error` returns `ProcessCrash` for "Broken pipe", "EOF while parsing", "process died" strings [owner:api-engineer]
- [x] [3.7] Unit test: `classify_error` returns `AuthFailure` for "Not logged in" and "auth" strings [owner:api-engineer]
- [x] [3.8] Unit test: `user_message` returns non-empty, no-emoji string for every `NovaError` variant at attempt 1 and at `max_attempts` [owner:api-engineer]
- [x] [3.9] Unit test: `retry_keyboard` returns `InlineKeyboard` with one row, one button, `callback_data` starts with `"retry:"` [owner:api-engineer]
- [x] [3.10] Existing tests pass [owner:api-engineer]
- [ ] [3.11] Manual gate: trigger a timeout — Telegram shows "retrying" progress, then final message with Retry button [user]
- [ ] [3.12] Manual gate: trigger auth failure — no retry, immediate error message, no Retry button [user]
