# Implementation Tasks

<!-- beads:epic:TBD -->

## ActionType Variant

- [x] [1.1] [P-1] Add `ChannelSend` variant to `ActionType` enum in `crates/nv-core/src/types.rs` [owner:api-engineer]

## Tool Definitions

- [x] [2.1] [P-1] Add `list_channels` tool definition to `register_tools()` in tools.rs — no input params, description: "List available messaging channels and their connection status" [owner:api-engineer]
- [x] [2.2] [P-1] Add `send_to_channel` tool definition to `register_tools()` in tools.rs — params: `channel` (string, required), `message` (string, required), `recipient` (string, optional — required for email channel). Description: "Send a message to a specific channel (telegram/discord/teams/email). Requires confirmation. For email, the recipient parameter is required." [owner:api-engineer]

## Thread ChannelRegistry to Dispatch

- [x] [3.1] [P-1] Add `channels: &ChannelRegistry` parameter to `execute_tool_send()` signature in tools.rs [owner:api-engineer]
- [x] [3.2] [P-1] Update `execute_tool_send` call site in worker.rs (~line 838) to pass `&deps.channels` [owner:api-engineer]
- [x] [3.3] [P-2] Add `channels: &ChannelRegistry` parameter to `execute_tool` (dead-code variant) for consistency [owner:api-engineer]

## Tool Dispatch

- [x] [4.1] [P-1] Add `list_channels` dispatch arm to `execute_tool_send` — iterate `channels.iter()`, format each as `"- {name} (connected)"`, return `ToolResult::Immediate` [owner:api-engineer]
- [x] [4.2] [P-1] Add `send_to_channel` dispatch arm to `execute_tool_send` — validate channel exists in registry, validate message non-empty, validate recipient present if channel is "email", build description string, return `ToolResult::PendingAction { description, action_type: ActionType::ChannelSend, payload: input.clone() }` [owner:api-engineer]
- [x] [4.3] [P-2] Add matching dispatch arms to `execute_tool` (dead-code variant) [owner:api-engineer]

## Confirmed Action Executor

- [x] [5.1] [P-1] Add `execute_channel_send` function in tools.rs — takes `channels: &ChannelRegistry` and `payload: &serde_json::Value`, deserializes channel/message/recipient, looks up channel in registry, calls `channel.send_message(OutboundMessage { channel, content: message, reply_to: None, keyboard: None })`, returns confirmation string [owner:api-engineer]
- [x] [5.2] [P-1] Add `ActionType::ChannelSend` arm to the confirmed-action dispatch in worker.rs — call `execute_channel_send` with `&deps.channels` and the action payload [owner:api-engineer]

## Verify

- [x] [6.1] `cargo build` passes [owner:api-engineer]
- [x] [6.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [6.3] `cargo test` — existing tests pass [owner:api-engineer]
