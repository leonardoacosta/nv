# Proposal: Add Anthropic API Client

## Change ID
`add-anthropic-api-client`

## Summary

Direct Anthropic Messages API client in Rust (`reqwest` + SSE) as a fallback when the CC
CLI persistent session is unavailable or unreliable. Bypasses the CC CLI entirely; calls the
API directly with streaming and tool-use support.

## Context
- New module: `crates/nv-daemon/src/anthropic.rs`
- Related: `crates/nv-daemon/src/claude.rs` — existing CC CLI interaction layer (type-compatible)
- Phase: Wave 3a (Direct API Fallback, conditional on CC CLI remaining unreliable)
- Depended on by: `native-tool-use-protocol` spec

## Motivation

The CC CLI persistent session path (`claude.rs`) is the primary inference layer, but it has
demonstrated reliability problems: process crashes, stdin/stdout framing errors, and
unpredictable hangs during tool-use agentic turns. When these occur, Nova has no fallback
and the user gets silence.

A direct Anthropic API client eliminates the CLI subprocess boundary entirely:

1. **No process management** — HTTP is stateless; no child process to keep alive or restart
2. **Full tool-use protocol** — send `tools` array and handle `tool_use` content blocks natively
3. **Streaming** — SSE gives incremental text output; latency matches or beats CLI path
4. **Retry semantics** — direct HTTP makes backoff straightforward (429/529 response codes)
5. **Feature parity with claude.rs** — reuses existing `Message`, `ContentBlock`, `StopReason`,
   `Usage`, and `ToolDefinition` types; the agent loop needs no changes to adopt the fallback

## Requirements

### Req-1: AnthropicClient Struct

```rust
pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: &str, model: &str) -> Self;
}
```

- `http` client built with default TLS; configured with `Accept: text/event-stream`
- `model` stored as `String`; default value `claude-sonnet-4-20250514` used when caller
  passes an empty string (or the config default is applied by the caller)

### Req-2: send_message

```rust
pub async fn send_message(
    &self,
    messages: Vec<Message>,
    system: &str,
    tools: Vec<ToolDefinition>,
) -> Result<ApiResponse, ApiError>
```

- Sends a POST to `https://api.anthropic.com/v1/messages` with `stream: true`
- Request body: `{"model","max_tokens":8192,"system","messages","tools"}`
- `anthropic-version: 2023-06-01` header required
- Returns a fully-assembled `ApiResponse` (same type used by `ClaudeSession`)
- Tools array is omitted from the request when `tools` is empty (avoids API validation error)

### Req-3: SSE Streaming

Parse the SSE event stream from the response body using `reqwest`'s `bytes_stream()` and
`futures_util::StreamExt`. The implementation must handle the following Anthropic SSE events:

| SSE event type | Action |
|---|---|
| `content_block_start` (type=text) | Begin accumulating text |
| `content_block_delta` (delta.type=text_delta) | Append `delta.text` |
| `content_block_start` (type=tool_use) | Record `id`, `name`; begin accumulating `input` JSON |
| `content_block_delta` (delta.type=input_json_delta) | Append `delta.partial_json` |
| `content_block_stop` | Finalise the current block |
| `message_delta` | Capture `stop_reason`, `output_tokens` |
| `message_start` | Capture `usage.input_tokens`, `id` |
| `message_stop` | End of stream |
| `error` | Return `ApiError::CliError` with the error message from the event |

No third-party SSE crate required — parse `data: {...}` lines directly from the byte stream.

### Req-4: Tool-Use Protocol

When the API returns content blocks of type `tool_use`:
- Deserialise accumulated `input` JSON string into `serde_json::Value`
- Emit a `ContentBlock::ToolUse { id, name, input }` in the returned `ApiResponse`
- `stop_reason` must be `StopReason::ToolUse` when at least one `tool_use` block is present

Tool results are provided by the caller as `Message::tool_results(...)` in the `messages` vec
(same contract as the CC CLI path). The client itself does not execute tools.

### Req-5: Retry with Exponential Backoff

On HTTP 429 (rate limited) or 529 (API overloaded):
- Retry up to 3 times
- Base delay: 1s, multiplied by `2^attempt` (1s, 2s, 4s)
- Log each retry at `WARN` level with attempt number and status code
- After 3 retries, return `ApiError::CliError` with the final status

All other 4xx/5xx responses return `ApiError::CliError` immediately (no retry).

### Req-6: API Key from Environment

`ANTHROPIC_API_KEY` is read from the environment at construction time:

```rust
impl AnthropicClient {
    pub fn from_env(model: &str) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY not set")?;
        Ok(Self::new(&api_key, model))
    }
}
```

On the homelab, `ANTHROPIC_API_KEY` is injected via Doppler service token in the systemd
unit environment. No `.env` file or hardcoded key is used.

### Req-7: Model Config in nv.toml

The model used by the direct client is read from `[agent] model` in `nv.toml` (same field
already used by the CC CLI path). Default: `claude-sonnet-4-20250514`.

The `AnthropicClient` is constructed with the model string from `AgentConfig::model`; no new
config field is needed.

### Req-8: Module Declaration

```rust
// crates/nv-daemon/src/main.rs
pub mod anthropic;
```

No Cargo.toml changes required — `reqwest` (with `stream` feature) and `futures-util` are
already workspace dependencies in `nv-daemon`.

## Type Reuse

The following types from `crates/nv-daemon/src/claude.rs` are reused directly:

| Type | Used for |
|---|---|
| `Message` / `MessageContent` | Conversation history sent to the API |
| `ContentBlock` | Text and tool_use blocks in the assembled response |
| `ApiResponse` | Return type of `send_message` |
| `ApiError` | Error type (adds no new variants) |
| `StopReason` | `EndTurn` / `ToolUse` / `MaxTokens` |
| `Usage` | Token accounting in the assembled response |
| `ToolDefinition` (re-export from nv-core) | Tool schema sent to the API |

No new public types are introduced.

## Scope

**IN:**
- `anthropic.rs` module with `AnthropicClient`, SSE parsing, retry logic
- `from_env` constructor reading `ANTHROPIC_API_KEY`
- Unit tests: SSE parsing, retry logic, tool-use block assembly
- `mod anthropic;` declaration in `main.rs`

**OUT:**
- Automatic switchover from CC CLI to direct API (handled by `native-tool-use-protocol` spec)
- Streaming token-by-token push to Telegram (future enhancement)
- Rate-limit quota tracking / budget integration
- Any changes to `agent.rs` or the agent loop

## Impact

| File | Change |
|---|---|
| `crates/nv-daemon/src/anthropic.rs` | New: full module |
| `crates/nv-daemon/src/main.rs` | Add `pub mod anthropic;` |

## Risks

| Risk | Mitigation |
|---|---|
| SSE framing edge cases (multi-line data, empty lines) | Line-by-line parser; skip blank lines; only parse `data:` prefixed lines |
| `input_json_delta` arrives in chunks; partial JSON until `content_block_stop` | Accumulate into `String`, parse only after `stop` event |
| `ANTHROPIC_API_KEY` not set in systemd environment | `from_env` returns `Err`; daemon logs and skips fallback path |
| reqwest stream drops mid-response | Return partial error; caller retries the full turn |
| Model string mismatch (future model IDs) | Read from config; operator updates `nv.toml`, not code |
