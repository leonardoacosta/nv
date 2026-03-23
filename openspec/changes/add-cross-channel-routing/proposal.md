# Proposal: Cross-Channel Notification Routing

## Change ID
`add-cross-channel-routing`

## Summary

Add two new tools — `send_to_channel` and `list_channels` — that let Claude route messages to any
configured channel (Telegram, Discord, Teams, email) on demand. `send_to_channel` is a write
operation requiring PendingAction confirmation. `list_channels` is read-only and returns the
registry status.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/worker.rs` (SharedDeps already holds `channels: ChannelRegistry`)
- Related: `ChannelRegistry` is `HashMap<String, Arc<dyn Channel>>` (defined in `agent.rs:146`), `Channel` trait in `crates/nv-core/src/channel.rs` with `send_message(OutboundMessage)`, `OutboundMessage` in `crates/nv-core/src/types.rs`
- PendingAction pattern: used by `jira_create`, `ha_service_call`, `nexus_start_session` — returns `ToolResult::PendingAction` with description, `ActionType`, and payload; worker persists to state; user confirms via Telegram inline keyboard
- Depends on: nothing — uses existing channel infrastructure

## Motivation

Nova can currently only respond on the channel that initiated the conversation. There is no way for
Claude to proactively send a message to a different channel — e.g., sending a reminder to Discord
while the conversation is on Telegram, or routing an alert to email. The `ChannelRegistry` and
`Channel::send_message()` already exist but are not exposed as tools.

## Requirements

### Req-1: `list_channels` Tool (Read-Only)

Register a `list_channels` tool that returns the names and connection status of all channels in the
`ChannelRegistry`.

- No input parameters required
- Returns a formatted list: one line per channel with name and "connected" status
- Example output:
  ```
  Available channels:
  - telegram (connected)
  - discord (connected)
  - email (connected)
  ```
- Read-only — returns `ToolResult::Immediate`
- Requires passing `&ChannelRegistry` to `execute_tool_send`

### Req-2: `send_to_channel` Tool (PendingAction)

Register a `send_to_channel` tool that sends a message to a named channel. This is a write
operation — it must go through PendingAction confirmation before executing.

Parameters:
| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `channel` | string | yes | Target channel name (must exist in registry) |
| `message` | string | yes | Message content to send |
| `recipient` | string | no | Required for email channel; ignored by others |

Validation (at tool dispatch time, before PendingAction):
- `channel` must exist in the `ChannelRegistry` — fail with error if not found
- `message` must be non-empty
- If `channel` is `"email"`, `recipient` must be provided

PendingAction flow:
1. Tool returns `ToolResult::PendingAction` with a human-readable description (e.g., `Send to discord: "Don't forget the standup at 10am"`)
2. Worker persists the action and shows confirmation keyboard
3. On approval, execute the send via `channel.send_message(OutboundMessage { ... })`

### Req-3: Add `ChannelSend` to `ActionType`

Add a `ChannelSend` variant to `nv_core::types::ActionType` for the new pending action. This is
distinct from the existing `ChannelReply` (which is used for reply routing, not proactive sends).

### Req-4: Execute Confirmed Channel Send

Add an execution handler (alongside `execute_jira_action`) that:
1. Deserializes the payload to extract `channel`, `message`, and optional `recipient`
2. Looks up the channel in the registry
3. Calls `channel.send_message(OutboundMessage { channel, content: message, reply_to: None, keyboard: None })`
4. Returns a confirmation string (e.g., `"Message sent to discord"`)

### Req-5: Thread `ChannelRegistry` to `execute_tool_send`

The current `execute_tool_send` signature does not include the channel registry. Add
`channels: &ChannelRegistry` as a parameter so both new tools can access it. Update all call sites
(worker.rs passes `&deps.channels`).

## Scope
- **IN**: `list_channels` tool definition + dispatch, `send_to_channel` tool definition + dispatch with PendingAction, `ChannelSend` ActionType variant, confirmed-action executor, threading ChannelRegistry into execute_tool_send
- **OUT**: new channel implementations (uses existing registered channels only), channel health checks / reconnection logic, message formatting or rich media, batched/scheduled sends

## Impact
| Area | Change |
|------|--------|
| `crates/nv-core/src/types.rs` | Add `ChannelSend` to `ActionType` enum |
| `crates/nv-daemon/src/tools.rs` | Add 2 tool definitions to `register_tools()`, add dispatch arms to `execute_tool_send` + `execute_tool`, add `channels` param to both functions, add confirmed-action executor |
| `crates/nv-daemon/src/worker.rs` | Pass `&deps.channels` to `execute_tool_send` call site, add `ChannelSend` arm to confirmed-action dispatch |

## Risks
| Risk | Mitigation |
|------|-----------|
| Adding `channels` param to `execute_tool_send` touches all call sites | Only one call site in worker.rs (line ~838) and one in `execute_tool` (unused, `#[allow(dead_code)]`). Mechanical change. |
| Email channel requires `recipient` but others don't — inconsistent UX | Validate at dispatch time with a clear error message. Tool description documents the requirement. |
| User accidentally sends to wrong channel via confirmation fatigue | PendingAction description includes channel name and message preview (truncated). User sees exactly what will be sent and where. |
| `ChannelSend` vs existing `ChannelReply` confusion | Different semantics: `ChannelReply` is worker reply routing, `ChannelSend` is proactive cross-channel send. Names are distinct. |
