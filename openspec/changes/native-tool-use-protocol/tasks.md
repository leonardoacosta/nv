# native-tool-use-protocol — Tasks

<!-- beads:epic:TBD -->

## nv-core: ToolDefinition Serialization Helper

- [ ] [1.1] [P-1] Add `anthropic_json()` method to `ToolDefinition` in `crates/nv-core/src/tool.rs` — returns `serde_json::Value` with keys `name`, `description`, `input_schema` (snake_case, matching Anthropic API wire format) [owner:api-engineer]
- [ ] [1.2] [P-2] Add unit test in `nv-core` — verify `anthropic_json()` output matches `{ "name": "...", "description": "...", "input_schema": { "type": "object", ... } }` [owner:api-engineer]

## Cold-Start: Remove Tool Prose Augmentation

- [ ] [2.1] [P-1] In `send_messages_cold_start_with_image`: delete the `augmented_system` block that appends `## Available Tools\n\n...` prose and fence-block instructions to the system prompt — pass `system` unchanged [owner:api-engineer]
- [ ] [2.2] [P-1] Serialize `tools` to a JSON array using `ToolDefinition::anthropic_json()` for each entry; store in `Option<String>` — `None` when tools is empty [owner:api-engineer]
- [ ] [2.3] [P-1] Replace `--tools "Read,Glob,Grep,Bash(*)"` in `base_args` with `--tools-json <json>` when tools are non-empty; omit both flags entirely when tools is empty [owner:api-engineer]
- [ ] [2.4] [P-2] Update `tracing::info!` payload size log in cold-start path — log `tools_json_bytes` instead of deprecated `system_bytes` that included tool schemas [owner:api-engineer]
- [ ] [2.5] [P-2] Verify `--tools-json` availability at startup in `ClaudeClient::new()` — run `claude --help` and check for the flag; if absent, log a `tracing::warn!` and set a `fallback_prose_tools: bool` flag on the client [owner:api-engineer]
- [ ] [2.6] [P-3] Implement the fallback path: when `fallback_prose_tools` is true, use the old `augmented_system` approach so the daemon degrades gracefully on older CLI versions rather than silently dropping tool access [owner:api-engineer]

## Cold-Start: Native Response Parsing

- [ ] [3.1] [P-1] Remove `parse_tool_calls()` function (~250 lines) from `crates/nv-daemon/src/claude.rs` [owner:api-engineer]
- [ ] [3.2] [P-1] Remove `ToolCall` private struct (used only by `parse_tool_calls`) [owner:api-engineer]
- [ ] [3.3] [P-1] Update cold-start response assembly: instead of calling `parse_tool_calls(&cli_response.result)`, deserialize the `content` array from the CLI JSON response directly into `Vec<ContentBlock>` using serde [owner:api-engineer]
- [ ] [3.4] [P-1] Update `CliJsonResponse` struct to include `content: Vec<ContentBlock>` field (alongside the existing `result: String` fallback for backward compat); prefer `content` when present [owner:api-engineer]
- [ ] [3.5] [P-2] Preserve existing stop_reason derivation — map CLI `stop_reason` string to `StopReason` enum; if `content` contains any `ToolUse` block, override to `StopReason::ToolUse` regardless of string value [owner:api-engineer]

## Persistent Session: Tools at Spawn Time

- [ ] [4.1] [P-1] Update `SpawnConfig` struct — add `tools_json: Option<String>` field [owner:api-engineer]
- [ ] [4.2] [P-1] Update `spawn_persistent()` — when `config.tools_json` is `Some`, append `--tools-json <json>` to the subprocess args; remove the existing `--tools "Read,Glob,Grep,Bash(git:*)"` flag [owner:api-engineer]
- [ ] [4.3] [P-2] Update `PersistentSession::new()` to accept tools at construction time — serialize all `ToolDefinition` entries to JSON and store in `SpawnConfig.tools_json` [owner:api-engineer]
- [ ] [4.4] [P-2] In `PersistentSession::send_turn()`: detect when the caller's `tools` list differs from the spawn-time list (compare serialized JSON or tool name sets) — log `tracing::warn!` and set `inner.process = None` to force respawn with updated tools [owner:api-engineer]
- [ ] [4.5] [P-3] Update `build_stream_input()` — remove the `_tools` parameter (now unused, tools registered at spawn time); keep signature for backward compat but mark `#[allow(unused)]` on the parameter [owner:api-engineer]

## Direct HTTP Path (AnthropicClient Integration)

- [ ] [5.1] [P-1] In the `AnthropicClient::send_messages()` method (from `add-anthropic-api-client`): include `"tools": tools` in the request body JSON when tools is non-empty — use `ToolDefinition::anthropic_json()` for each entry [owner:api-engineer]
- [ ] [5.2] [P-2] Deserialize HTTP response `content` array directly into `Vec<ContentBlock>` — the existing serde `#[serde(tag = "type")]` enum already handles `text` and `tool_use` variants correctly [owner:api-engineer]
- [ ] [5.3] [P-2] Add `tool_choice` field support to `AnthropicClient` request builder — default to `{ "type": "auto" }` (Claude decides when to use tools); expose override for callers that need `"none"` (digest calls that should not invoke tools) [owner:api-engineer]

## Startup Validation

- [ ] [6.1] [P-2] Add `validate_tool_definitions(tools: &[ToolDefinition])` function — verify each `input_schema` is a JSON object with `"type": "object"` key; log `tracing::warn!` for any that fail validation (do not panic — degrade gracefully) [owner:api-engineer]
- [ ] [6.2] [P-2] Call `validate_tool_definitions()` in `ClaudeClient::new()` after tools are collected from `ToolRegistry::list_tools()` [owner:api-engineer]

## Cleanup

- [ ] [7.1] [P-2] Remove the `MARKER` / `CLOSE` fence-block constants and any remaining dead code paths that referenced `parse_tool_calls` [owner:api-engineer]
- [ ] [7.2] [P-3] Update `tracing::info!` in `send_turn` — log `tools_registered` count (from spawn config) instead of `tools.len()` per call [owner:api-engineer]
- [ ] [7.3] [P-3] Add `#[cfg(test)]` guard on `build_prompt()` (already present) — confirm it is not referenced from any non-test path after this change [owner:api-engineer]

## Verify

- [ ] [8.1] [P-1] `cargo build` passes with no errors [owner:api-engineer]
- [ ] [8.2] [P-1] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [8.3] [P-1] `cargo test` — existing agent loop and claude.rs unit tests pass [owner:api-engineer]
- [ ] [8.4] [P-1] Integration test: send a message that requires a tool call via the cold-start path; verify the tool is dispatched and the result returned without relying on fence-block parsing [owner:api-engineer]
- [ ] [8.5] [P-2] Measure cold-start system prompt size before and after — verify `system_bytes` drops from ~49KB to ~5KB in the trace log [owner:api-engineer]
- [ ] [8.6] [P-2] Test multi-tool batching: craft a message that causes Claude to call two tools in a single turn; verify both `ContentBlock::ToolUse` entries are dispatched and both `tool_result` blocks returned [owner:api-engineer]
- [ ] [8.7] [P-2] Test `tool_choice: "none"` path — verify digest calls that pass `tool_choice: none` receive a plain text response with no tool dispatch [owner:api-engineer]
- [ ] [8.8] [P-3] Snapshot test: capture `base_args` for a cold-start call with 3 tools — verify `--tools-json` is present and `--tools "Read,Glob..."` is absent [owner:api-engineer]
- [ ] [8.9] [user] Manual test: send "check docker status" via Telegram; verify response without fence-block artifacts in the text [owner:user]
- [ ] [8.10] [user] Manual test: send "what's my next calendar event and any open Stripe invoices" — verify Nova batches both tool calls in one turn [owner:user]
