# agent-loop — Tasks

## mpsc Channel Setup
- [x] Create `mpsc::unbounded_channel<Trigger>()` in daemon main.rs
- [x] Pass `Sender` clone to Telegram listener (connect to spec-3 output)
- [x] Pass `Receiver` to `AgentLoop::new()`
- [x] Verify channel shutdown: when all senders drop, receiver returns `None`

## AgentLoop Struct
- [x] Define `AgentLoop` struct in `nv-daemon/src/agent.rs` with fields: config, client, trigger_rx, channels, conversation_history, system_prompt, tool_definitions, last_activity
- [x] Implement `AgentLoop::new()` constructor — loads system prompt, registers tool definitions, initializes empty history
- [x] Define `ChannelRegistry` as `HashMap<String, Arc<dyn Channel>>` for outbound routing
- [x] Register Telegram channel in registry at startup

## Batch Drain Logic
- [x] Implement `drain_triggers()` — blocking `recv()` for first trigger, then `try_recv()` loop
- [x] Log batch size with `tracing::info!`
- [x] Handle channel closed (return empty vec to signal shutdown)
- [x] Unit test: send 5 triggers, call drain, verify all 5 returned in batch

## Claude API Client
- [x] Define `ClaudeClient` struct in `nv-daemon/src/claude.rs` with reqwest::Client, api_key, model, max_tokens
- [x] Implement `ClaudeClient::new()` — reads `ANTHROPIC_API_KEY` from env, model from config
- [x] Implement `send_messages()` — POST to `https://api.anthropic.com/v1/messages`
- [x] Set required headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- [x] Define `ApiResponse`, `ContentBlock`, `StopReason`, `Usage` serde types
- [x] Define `Message` struct with role (user/assistant) and content (Vec<ContentBlock> or String)
- [x] Handle HTTP error responses — map status codes to `ApiError` variants

## System Prompt
- [x] Write default system prompt as embedded string constant in `nv-daemon/src/agent.rs`
- [x] Load override from `~/.nv/system-prompt.md` if file exists, fall back to default
- [x] System prompt covers: identity, autonomy rules, available tools, response format, context explanation
- [x] Unit test: verify default prompt loads when no file exists

## Tool Definitions
- [x] Define `ToolDefinition` struct (name, description, input_schema as serde_json::Value)
- [x] Create `register_tools()` function that returns `Vec<ToolDefinition>` for all initial tools
- [x] Define schema for `read_memory` tool: `{ topic: string }`
- [x] Define schema for `search_memory` tool: `{ query: string }`
- [x] Define schema for `write_memory` tool: `{ topic: string, content: string }`
- [x] Define schema for `query_jira` tool: `{ jql: string }`
- [x] Define schema for `query_nexus` tool: `{}` (no parameters)
- [x] Serialize tool definitions to Anthropic API `tools` format in request body

## Tool Execution Loop
- [x] Implement `run_tool_loop()` — loop while `stop_reason == ToolUse`
- [x] Parse `ContentBlock::ToolUse` blocks from response, extract id/name/input
- [x] Implement `execute_tool()` dispatch — match on tool name, call appropriate handler
- [x] Stub implementations for all 5 tools (return placeholder strings for spec-4, real implementations come in spec-5/6)
- [x] Format `tool_result` content blocks per Anthropic API spec (tool_use_id, content, is_error)
- [x] Append assistant message (with tool_use blocks) to conversation history
- [x] Append user message (with tool_result blocks) to conversation history
- [x] Send continued conversation back to Claude
- [x] Handle tool execution errors: return error string with `is_error: true`, let Claude adapt
- [x] Safety limit: max 10 tool loop iterations per agent cycle to prevent runaway

## Trigger Formatting
- [x] Implement `format_trigger_batch()` — convert Vec<Trigger> to structured text
- [x] Format `Trigger::Message` with channel, timestamp, sender, content
- [x] Format `Trigger::Cron` with event type
- [x] Format `Trigger::NexusEvent` with event details
- [x] Format `Trigger::CliCommand` with command text

## Response Routing
- [x] Implement `route_response()` — extract text from ContentBlocks, determine target channel
- [x] Route to source channel for message triggers (reply to where it came from)
- [x] Default to Telegram for cron/nexus/cli triggers
- [x] Set `reply_to` field when responding to a specific message
- [x] Handle empty response (no text blocks) — log warning, no send

## PendingAction Flow
- [ ] Implement `parse_pending_action()` — detect action drafts in Claude's response
- [ ] Create `PendingAction` with UUID, description, payload, status
- [ ] Write pending action to `~/.nv/state/pending-actions.json`
- [ ] Send Telegram message with inline keyboard (Confirm / Edit / Cancel)
- [ ] Format callback data as `action:{verb}:{action_id}`

## Context Window Management
- [x] Implement `truncate_history()` — enforce MAX_HISTORY_TURNS (20) and MAX_HISTORY_CHARS (50000)
- [x] Drop oldest turns first when over budget
- [x] Always keep at least the 2 most recent turns
- [x] Implement session timeout: clear history after 10 minutes of inactivity
- [x] Track `last_activity: Instant` — update on each successful Claude response

## Error Handling
- [x] Define `ApiError` enum with thiserror: HttpError, RateLimited, Network, Deserialize
- [x] Handle HTTP 429: parse `retry-after` header, sleep, retry once
- [x] Handle HTTP 5xx: retry up to 3 times with exponential backoff (1s, 2s, 4s)
- [x] Handle HTTP 401: log error, notify on Telegram, no retry
- [x] Handle network errors: retry 3 times with backoff
- [x] Handle malformed JSON response: log body at error level, return generic error
- [x] Handle `stop_reason: max_tokens`: log warning, use partial response
- [x] Implement `handle_api_error()` — send error notification to Telegram for persistent failures
- [x] Agent loop must never panic — wrap each cycle in error handling, continue on next trigger

## Main Loop Integration
- [x] Implement `AgentLoop::run()` as the top-level loop: drain → build → call → route
- [ ] Inject memory context into messages before each Claude call
- [x] Call `maybe_reset_session()` before each cycle to check inactivity timeout
- [x] Wire into daemon `main.rs`: spawn agent loop as tokio task after Telegram listener
- [x] Implement graceful shutdown: agent loop exits when trigger channel closes (all senders dropped)
