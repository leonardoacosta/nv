# agent-loop — Tasks

## mpsc Channel Setup
- [ ] Create `mpsc::unbounded_channel<Trigger>()` in daemon main.rs
- [ ] Pass `Sender` clone to Telegram listener (connect to spec-3 output)
- [ ] Pass `Receiver` to `AgentLoop::new()`
- [ ] Verify channel shutdown: when all senders drop, receiver returns `None`

## AgentLoop Struct
- [ ] Define `AgentLoop` struct in `nv-daemon/src/agent.rs` with fields: config, client, trigger_rx, channels, conversation_history, system_prompt, tool_definitions, last_activity
- [ ] Implement `AgentLoop::new()` constructor — loads system prompt, registers tool definitions, initializes empty history
- [ ] Define `ChannelRegistry` as `HashMap<String, Arc<dyn Channel>>` for outbound routing
- [ ] Register Telegram channel in registry at startup

## Batch Drain Logic
- [ ] Implement `drain_triggers()` — blocking `recv()` for first trigger, then `try_recv()` loop
- [ ] Log batch size with `tracing::info!`
- [ ] Handle channel closed (return empty vec to signal shutdown)
- [ ] Unit test: send 5 triggers, call drain, verify all 5 returned in batch

## Claude API Client
- [ ] Define `ClaudeClient` struct in `nv-core/src/claude.rs` with reqwest::Client, api_key, model, max_tokens
- [ ] Implement `ClaudeClient::new()` — reads `ANTHROPIC_API_KEY` from env, model from config
- [ ] Implement `send_messages()` — POST to `https://api.anthropic.com/v1/messages`
- [ ] Set required headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- [ ] Define `ApiResponse`, `ContentBlock`, `StopReason`, `Usage` serde types
- [ ] Define `Message` struct with role (user/assistant) and content (Vec<ContentBlock> or String)
- [ ] Handle HTTP error responses — map status codes to `ApiError` variants

## System Prompt
- [ ] Write default system prompt as embedded string constant in `nv-core/src/prompts.rs`
- [ ] Load override from `~/.nv/system-prompt.md` if file exists, fall back to default
- [ ] System prompt covers: identity, autonomy rules, available tools, response format, context explanation
- [ ] Unit test: verify default prompt loads when no file exists

## Tool Definitions
- [ ] Define `ToolDefinition` struct (name, description, input_schema as serde_json::Value)
- [ ] Create `register_tools()` function that returns `Vec<ToolDefinition>` for all initial tools
- [ ] Define schema for `read_memory` tool: `{ topic: string }`
- [ ] Define schema for `search_memory` tool: `{ query: string }`
- [ ] Define schema for `write_memory` tool: `{ topic: string, content: string }`
- [ ] Define schema for `query_jira` tool: `{ jql: string }`
- [ ] Define schema for `query_nexus` tool: `{}` (no parameters)
- [ ] Serialize tool definitions to Anthropic API `tools` format in request body

## Tool Execution Loop
- [ ] Implement `run_tool_loop()` — loop while `stop_reason == ToolUse`
- [ ] Parse `ContentBlock::ToolUse` blocks from response, extract id/name/input
- [ ] Implement `execute_tool()` dispatch — match on tool name, call appropriate handler
- [ ] Stub implementations for all 5 tools (return placeholder strings for spec-4, real implementations come in spec-5/6)
- [ ] Format `tool_result` content blocks per Anthropic API spec (tool_use_id, content, is_error)
- [ ] Append assistant message (with tool_use blocks) to conversation history
- [ ] Append user message (with tool_result blocks) to conversation history
- [ ] Send continued conversation back to Claude
- [ ] Handle tool execution errors: return error string with `is_error: true`, let Claude adapt
- [ ] Safety limit: max 10 tool loop iterations per agent cycle to prevent runaway

## Trigger Formatting
- [ ] Implement `format_trigger_batch()` — convert Vec<Trigger> to structured text
- [ ] Format `Trigger::Message` with channel, timestamp, sender, content
- [ ] Format `Trigger::Cron` with event type
- [ ] Format `Trigger::NexusEvent` with event details
- [ ] Format `Trigger::CliCommand` with command text

## Response Routing
- [ ] Implement `route_response()` — extract text from ContentBlocks, determine target channel
- [ ] Route to source channel for message triggers (reply to where it came from)
- [ ] Default to Telegram for cron/nexus/cli triggers
- [ ] Set `reply_to` field when responding to a specific message
- [ ] Handle empty response (no text blocks) — log warning, no send

## PendingAction Flow
- [ ] Implement `parse_pending_action()` — detect action drafts in Claude's response
- [ ] Create `PendingAction` with UUID, description, payload, status
- [ ] Write pending action to `~/.nv/state/pending-actions.json`
- [ ] Send Telegram message with inline keyboard (Confirm / Edit / Cancel)
- [ ] Format callback data as `action:{verb}:{action_id}`

## Context Window Management
- [ ] Implement `truncate_history()` — enforce MAX_HISTORY_TURNS (20) and MAX_HISTORY_CHARS (50000)
- [ ] Drop oldest turns first when over budget
- [ ] Always keep at least the 2 most recent turns
- [ ] Implement session timeout: clear history after 10 minutes of inactivity
- [ ] Track `last_activity: Instant` — update on each successful Claude response

## Error Handling
- [ ] Define `ApiError` enum with thiserror: HttpError, RateLimited, Network, Deserialize
- [ ] Handle HTTP 429: parse `retry-after` header, sleep, retry once
- [ ] Handle HTTP 5xx: retry up to 3 times with exponential backoff (1s, 2s, 4s)
- [ ] Handle HTTP 401: log error, notify on Telegram, no retry
- [ ] Handle network errors: retry 3 times with backoff
- [ ] Handle malformed JSON response: log body at error level, return generic error
- [ ] Handle `stop_reason: max_tokens`: log warning, use partial response
- [ ] Implement `handle_api_error()` — send error notification to Telegram for persistent failures
- [ ] Agent loop must never panic — wrap each cycle in error handling, continue on next trigger

## Main Loop Integration
- [ ] Implement `AgentLoop::run()` as the top-level loop: drain → build → call → route
- [ ] Inject memory context into messages before each Claude call
- [ ] Call `maybe_reset_session()` before each cycle to check inactivity timeout
- [ ] Wire into daemon `main.rs`: spawn agent loop as tokio task after Telegram listener
- [ ] Implement graceful shutdown: agent loop exits when trigger channel closes (all senders dropped)
