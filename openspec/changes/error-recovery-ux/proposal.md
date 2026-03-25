# error-recovery-ux

## Summary

Replace generic error text in Telegram with structured error types, user-friendly messages per error class, automatic retry with exponential backoff for transient errors (429, 529, timeout), and a "Retry" inline button on final failure. Structured error context is logged for debugging.

## Motivation

When Claude fails mid-conversation, Nova currently sends raw error strings (e.g., `⚠ CLI execution failed: HTTP request failed: ...`) directly to Telegram. These are meaningless to the operator and provide no path forward. The fix distinguishes error classes, gives each a human-readable message with actionable guidance, retries transient failures silently before surfacing to the user, and offers a one-tap retry on final failure — all while logging full context for debugging.

## Current State

Error handling in `worker.rs` (`call_claude_with_fallback` error branch, lines ~1231-1278) is a set of `error_str.contains(...)` substring matches with emoji-prefixed strings wired directly to `OutboundMessage`. The `ApiError` enum in `claude.rs` has four variants: `CliError`, `AuthError`, `Deserialize`, `Process`. `AnthropicClient::send_with_retry` in `anthropic.rs` already performs 3 retries with 1s/2s/4s backoff for HTTP 429/529, but this logic is internal to `AnthropicClient` — the CLI path (`PersistentSession`) has no equivalent retry handling in `worker.rs`.

## Design

### 1. Structured Error Classification

Add a `NovaError` enum in a new file `crates/nv-daemon/src/error_recovery.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum NovaError {
    /// HTTP 429 or CLI rate-limit message.
    RateLimit { retry_after_secs: Option<u64> },
    /// HTTP 529 / "overloaded" from Anthropic.
    ApiOverloaded,
    /// Network timeout or process timeout.
    Timeout,
    /// Auth failure — credentials invalid/missing.
    AuthFailure,
    /// Process crash (Broken pipe, EOF, process died).
    ProcessCrash,
    /// Unclassified error.
    Unknown { message: String },
}
```

A `classify_error(e: &anyhow::Error) -> NovaError` function maps error strings to variants using the existing substring patterns plus HTTP status code detection from `ApiError` variants.

### 2. User-Facing Messages

A `user_message(error: &NovaError, attempt: u32, max_attempts: u32) -> String` function returns plain text (no raw errors, no emojis per project convention):

| Error class | Message |
|---|---|
| `RateLimit` | "I've hit my usage limit — retrying in a moment." (during retry) / "I've hit my usage limit. Try again shortly." (final) |
| `ApiOverloaded` | "The API is overloaded — retrying." (during retry) / "The API is still overloaded. Try again in a moment." (final) |
| `Timeout` | "That's taking longer than expected — retrying." / "That took too long. Try a shorter request." (final) |
| `AuthFailure` | "Authentication issue — check Claude CLI credentials." (no retry — not transient) |
| `ProcessCrash` | "Something went wrong — retrying." / "Something went wrong. Please try again." (final) |
| `Unknown` | "Something went wrong. Please try again." |

### 3. Retry Logic in worker.rs

Extract the Claude call into a retry loop in `run_worker` (the function that runs a `WorkerTask`):

```
const MAX_WORKER_RETRIES: u32 = 2;  // up to 3 total attempts
backoff: [2s, 5s]
retryable: RateLimit, ApiOverloaded, Timeout, ProcessCrash
non-retryable: AuthFailure, Unknown
```

On each retryable failure before final attempt:
1. Log `tracing::warn!` with structured fields: `error_class`, `attempt`, `worker_id`
2. Send a brief "retrying" message to Telegram (the per-attempt message from step 2)
3. Sleep the backoff interval
4. Re-call `call_claude_with_fallback`

On final failure (all attempts exhausted or non-retryable):
1. Emit `WorkerEvent::Error`
2. Set red-X reaction on original message
3. Send the final user message to Telegram with a "Retry" inline button

### 4. Retry Inline Button

Add a new callback prefix `retry:` to the callback routing in `callbacks.rs`. The button payload is `retry:{task_slug}` where `task_slug` is the existing `WorkerTask.slug`.

When the user taps "Retry":
1. The callback is received in the Telegram poll loop
2. The original trigger message is reconstructed from the slug (or the bot sends a new `Trigger::Message` with the original `InboundMessage` text derived from the slug)
3. This is dispatched back through the orchestrator as a new `WorkerTask` at `Priority::High`

The "Retry" button is only sent on **final** failure — not on intermediate retry attempts (which show inline progress text instead).

Button definition:

```rust
pub fn retry_keyboard(task_slug: &str) -> InlineKeyboard {
    InlineKeyboard {
        rows: vec![vec![InlineButton {
            text: "Retry".to_string(),
            callback_data: format!("retry:{task_slug}"),
        }]],
    }
}
```

### 5. Structured Error Logging

`tracing::error!` at final failure includes structured fields:

```rust
tracing::error!(
    worker_id = %task_id,
    error_class = ?error_class,
    attempt = max_attempts,
    raw_error = %e,
    "worker failed after all retries"
);
```

This replaces the existing unstructured `tracing::error!(worker_task = %task_id, error = %e, "worker failed")`.

### 6. AnthropicClient Backoff Alignment

The existing `AnthropicClient::send_with_retry` (3 retries, 1s/2s/4s) handles 429/529 at the HTTP layer. The new worker-level retry is a **higher-level** retry that fires after the `AnthropicClient` has already exhausted its retries. The two layers are complementary:

- `AnthropicClient` retries: fast, HTTP-level, transparent to the worker
- Worker retries: slower, with user notification, cover process crashes and timeouts that `AnthropicClient` cannot see

No changes to `AnthropicClient` retry logic are needed.

## Files Changed

| File | Change |
|---|---|
| `crates/nv-daemon/src/error_recovery.rs` | New — `NovaError`, `classify_error`, `user_message`, `retry_keyboard` |
| `crates/nv-daemon/src/worker.rs` | Replace error branch with retry loop using `error_recovery` |
| `crates/nv-daemon/src/callbacks.rs` | Add `retry:` prefix routing |
| `crates/nv-daemon/src/lib.rs` | `pub mod error_recovery;` |

## Dependencies

None (depends on: none).

## Out of Scope

- Persisting retry state across daemon restarts
- Per-error-class retry budget (all retryable classes share the same `MAX_WORKER_RETRIES`)
- Retry button for tool-level errors (only Claude API call failures)
- Streaming partial response recovery

## Verification

- `cargo build` passes for all workspace members
- `cargo clippy -- -D warnings` passes
- Unit tests: `classify_error` returns correct variant for each known error string
- Unit tests: `user_message` returns correct string for each `NovaError` variant at attempt 1 vs final
- Unit tests: `retry_keyboard` returns correct `InlineKeyboard` with `retry:` callback data
- Manual: trigger a timeout error — Telegram shows "retrying" message, then final message with Retry button
- Manual: trigger auth failure — no retry, immediate final message, no Retry button
- Existing tests pass
