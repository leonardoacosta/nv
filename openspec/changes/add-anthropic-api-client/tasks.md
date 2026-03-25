# Implementation Tasks

<!-- beads:epic:TBD -->

## Module Skeleton

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/anthropic.rs` — empty module with top-level doc comment and `use` imports (reqwest, futures_util, serde_json, anyhow, tracing; reuse claude.rs types: Message, ContentBlock, ApiResponse, ApiError, StopReason, Usage, ToolDefinition) [owner:api-engineer]
- [ ] [1.2] [P-1] Define `AnthropicClient` struct — fields: `http: reqwest::Client`, `api_key: String`, `model: String` [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `AnthropicClient::new(api_key: &str, model: &str) -> Self` — build reqwest Client with default TLS settings [owner:api-engineer]
- [ ] [1.4] [P-1] Implement `AnthropicClient::from_env(model: &str) -> Result<Self>` — read `ANTHROPIC_API_KEY` from env, return `Err` with clear message if unset [owner:api-engineer]
- [ ] [1.5] [P-1] Add `pub mod anthropic;` to `crates/nv-daemon/src/main.rs` [owner:api-engineer]

## Request Building

- [ ] [2.1] [P-1] Implement private `build_request_body` — serialize `{"model","max_tokens":8192,"system","messages","tools","stream":true}` to `serde_json::Value`; omit `tools` key entirely when `tools` vec is empty [owner:api-engineer]
- [ ] [2.2] [P-1] Add `anthropic-version: 2023-06-01` and `x-api-key` headers to POST request; set `Content-Type: application/json` and `Accept: text/event-stream` [owner:api-engineer]

## SSE Streaming Parser

- [ ] [3.1] [P-1] Implement `parse_sse_stream` — consume `reqwest::Response` bytes stream, split on newlines, collect `data: {...}` lines, skip blank lines and `: ping` keep-alives [owner:api-engineer]
- [ ] [3.2] [P-1] Handle `message_start` event — extract `message.id` and `usage.input_tokens` from event data [owner:api-engineer]
- [ ] [3.3] [P-1] Handle `content_block_start` (type=text) — push new text accumulator [owner:api-engineer]
- [ ] [3.4] [P-1] Handle `content_block_delta` (delta.type=text_delta) — append `delta.text` to current text accumulator [owner:api-engineer]
- [ ] [3.5] [P-1] Handle `content_block_start` (type=tool_use) — record `id` and `name`, push new tool input JSON accumulator [owner:api-engineer]
- [ ] [3.6] [P-1] Handle `content_block_delta` (delta.type=input_json_delta) — append `delta.partial_json` string to current tool input accumulator [owner:api-engineer]
- [ ] [3.7] [P-1] Handle `content_block_stop` — finalise current block: for text emit `ContentBlock::Text`; for tool_use parse accumulated JSON string into `serde_json::Value` and emit `ContentBlock::ToolUse { id, name, input }` [owner:api-engineer]
- [ ] [3.8] [P-1] Handle `message_delta` — extract `delta.stop_reason` and `usage.output_tokens` [owner:api-engineer]
- [ ] [3.9] [P-1] Handle `error` SSE event — return `ApiError::CliError` with the error type and message from the event payload [owner:api-engineer]
- [ ] [3.10] [P-2] Assemble final `ApiResponse` — set `id`, `content: Vec<ContentBlock>`, `stop_reason: StopReason` (map `"end_turn"` / `"tool_use"` / `"max_tokens"` strings), `usage: Usage` [owner:api-engineer]

## Retry Logic

- [ ] [4.1] [P-1] Implement `send_with_retry` — wrap the HTTP POST + SSE parse; on 429 or 529 response status, sleep exponential backoff (1s, 2s, 4s) and retry up to 3 times; log each retry at `WARN` with attempt number and status [owner:api-engineer]
- [ ] [4.2] [P-2] On non-retryable 4xx/5xx (e.g. 400 invalid request, 401 auth error): return `ApiError::CliError` immediately without retry, include status code and response body in the error message [owner:api-engineer]

## Public API

- [ ] [5.1] [P-1] Implement `AnthropicClient::send_message(messages, system, tools) -> Result<ApiResponse, ApiError>` — delegates to `send_with_retry`; entry point for callers [owner:api-engineer]

## Unit Tests

- [ ] [6.1] [P-2] Test `parse_sse_stream` with a fixture of a complete text-only SSE event sequence — verify assembled `ApiResponse` has correct text, stop_reason=EndTurn, usage counts [owner:api-engineer]
- [ ] [6.2] [P-2] Test `parse_sse_stream` with a fixture of a tool-use SSE sequence — verify `ContentBlock::ToolUse` with correct id/name/input and stop_reason=ToolUse [owner:api-engineer]
- [ ] [6.3] [P-2] Test `parse_sse_stream` with an `error` event — verify `ApiError::CliError` returned with event message [owner:api-engineer]
- [ ] [6.4] [P-2] Test `build_request_body` with empty tools vec — verify `tools` key absent from JSON [owner:api-engineer]
- [ ] [6.5] [P-3] Test `from_env` with `ANTHROPIC_API_KEY` unset — verify `Err` returned with descriptive message [owner:api-engineer]

## Verify

- [ ] [7.1] `cargo build -p nv-daemon` passes with no new errors [owner:api-engineer]
- [ ] [7.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [7.3] `cargo test -p nv-daemon anthropic` — all unit tests in the anthropic module pass [owner:api-engineer]
- [ ] [7.4] [user] Manual smoke test: construct `AnthropicClient::from_env` in a binary or test harness, call `send_message` with a simple text prompt, verify `ApiResponse` content is non-empty and stop_reason=EndTurn [owner:api-engineer]
- [ ] [7.5] [user] Manual tool-use test: call `send_message` with a tool definition and a prompt that triggers it, verify `ContentBlock::ToolUse` present in response and stop_reason=ToolUse [owner:api-engineer]
