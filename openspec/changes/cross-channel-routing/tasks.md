# Implementation Tasks

<!-- beads:epic:nv-q9yd -->

## Module: tools/channels.rs

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/tools/channels.rs` — define `ChannelDirection` enum (`Inbound`, `Outbound`, `Bidirectional`) and `ChannelInfo` struct (`name: String`, `connected: bool`, `direction: ChannelDirection`) [owner:api-engineer]
- [ ] [1.2] [P-1] Implement `list_channels(registry: &ChannelRegistry) -> Result<String>` — iterate adapters, build `ChannelInfo` per adapter using static direction table (Req-5), format as aligned text table: `name | connected | direction` [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `send_to_channel(registry: &ChannelRegistry, channel: &str, message: &str) -> Result<PendingActionRequest>` — validate channel exists + supports outbound, return `PendingActionRequest` with description and payload; do NOT call `send_message` directly [owner:api-engineer]
- [ ] [1.4] [P-2] Implement `channels_tool_definitions() -> Vec<ToolDefinition>` — `list_channels` with empty input schema `{}`; `send_to_channel` with `{ channel: { type: "string" }, message: { type: "string" } }`, both required [owner:api-engineer]
- [ ] [1.5] [P-2] Add `pub mod channels;` to `crates/nv-daemon/src/tools/mod.rs` [owner:api-engineer]

## Daemon Integration

- [ ] [2.1] [P-1] Include `channels::channels_tool_definitions()` in the daemon's `daemon_tool_definitions()` return value (SharedDeps impl) [owner:api-engineer]
- [ ] [2.2] [P-1] Handle `"list_channels"` in `call_tool()` dispatch — call `channels::list_channels(&channel_registry)`, return `Value::String(output)` [owner:api-engineer]
- [ ] [2.3] [P-1] Handle `"send_to_channel"` in `call_tool()` dispatch — extract `channel` + `message` args, call `channels::send_to_channel(...)`, persist PendingAction, send Telegram confirmation keyboard [Confirm] [Cancel] [owner:api-engineer]
- [ ] [2.4] [P-2] Wire post-confirmation execution in callback handler — on `Approved`: look up payload, call `target_channel.send_message(OutboundMessage { body: message })`, mark `Executed`; on `Rejected`: mark `Rejected`; on timeout: mark `Expired` [owner:api-engineer]
- [ ] [2.5] [P-2] Pass live channel registry handle into the SharedDeps dispatch path so `list_channels` and `send_to_channel` can access adapters [owner:api-engineer]

## System Prompt & Audit

- [ ] [3.1] [P-2] Update `DEFAULT_SYSTEM_PROMPT` in `crates/nv-daemon/src/agent.rs` — add `list_channels` to the Reads line; add `send_to_channel` to the Writes (confirm first) line [owner:api-engineer]
- [ ] [3.2] [P-3] Add audit log entries in tool dispatch — `list_channels`: log `invoked, channels=N`; `send_to_channel`: log `channel=<name>, msg_len=N, action_id=<uuid>, status=<outcome>` [owner:api-engineer]

## Tests

- [ ] [4.1] [P-1] Unit test `list_channels` with mock registry containing telegram (connected=true), discord (connected=false), teams (connected=true) — assert output contains all three names and correct direction labels [owner:api-engineer]
- [ ] [4.2] [P-1] Unit test `send_to_channel` — valid channel returns `Ok(PendingActionRequest)` with correct description and payload [owner:api-engineer]
- [ ] [4.3] [P-2] Unit test `send_to_channel` — unknown channel name returns `Err` containing "not configured" [owner:api-engineer]
- [ ] [4.4] [P-2] Unit test `send_to_channel` — inbound-only channel returns `Err` containing "does not support outbound" [owner:api-engineer]
- [ ] [4.5] [P-2] Unit test `channels_tool_definitions` — assert 2 definitions returned, names are `"list_channels"` and `"send_to_channel"`, `send_to_channel` schema requires both `channel` and `message` [owner:api-engineer]

## Verify

- [ ] [5.1] `cargo build` passes [owner:api-engineer]
- [ ] [5.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [ ] [5.3] `cargo test` — all new unit tests pass, existing tests unaffected [owner:api-engineer]
- [ ] [5.4] [user] Manual: ask Nova "what channels are available?" via Telegram, verify `list_channels` response shows all configured adapters with correct connected/direction status [owner:leo]
- [ ] [5.5] [user] Manual: ask Nova "send 'hello from Telegram' to Discord" via Telegram, verify PendingAction confirmation appears, approve it, confirm message appears in Discord [owner:leo]
- [ ] [5.6] [user] Manual: ask Nova to send to a channel name that doesn't exist, verify immediate error response without a confirmation prompt [owner:leo]
