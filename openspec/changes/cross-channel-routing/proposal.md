# Proposal: Cross-Channel Routing Tools

## Change ID
`cross-channel-routing`

## Summary

Two new daemon-coupled tools: `send_to_channel(channel, message)` — Nova can send a message to
any configured channel (Telegram, Discord, Teams) with mandatory PendingAction confirmation before
delivery. `list_channels()` — returns every configured channel with its name, type, and connection
status. Enables Nova to route information between channels: forward a Teams thread summary to
Telegram, send a reminder to Discord, or push a digest excerpt to a specific channel on demand.
Every send requires Leo's approval via the obligation/PendingAction callback system.

## Context

- Extends: `crates/nv-daemon/src/tools/` (new `channels.rs` tool module),
  `crates/nv-daemon/src/worker.rs` (PendingAction dispatch for `send_to_channel`),
  `crates/nv-daemon/src/orchestrator.rs` (tool registration via `daemon_tool_definitions`)
- Related: `nv-core::channel::Channel` trait (`channel.rs`), existing channel adapters in
  `crates/nv-daemon/src/channels/` (telegram, discord, teams, imessage, email),
  `PendingAction` / `PendingStatus` in `state.rs`, obligation/callback flow in
  `obligation_detector.rs` + `obligation_store.rs`
- Depends on: `callback-handler-completion` (Wave 3) — the callback/approval flow that resolves
  PendingActions must be complete before `send_to_channel` can execute confirmations
- Roadmap: Phase 3 Wave 8, idea source `nv-2e6`

## Motivation

Nova can receive messages from five channels but cannot initiate cross-channel sends. Use cases
that are currently blocked:

- "Forward this Teams standup summary to my Telegram" — Leo asks Nova verbally; Nova must relay
  the content across channel boundaries.
- "Ping me on Discord when the deploy finishes" — a watcher fires, Nova formats the result, but
  has no way to target Discord specifically.
- "Send a morning reminder to Teams" — scheduled digest content destined for a non-Telegram
  channel.

`list_channels()` solves the discovery problem: Nova (and Leo) need a single tool call to see
what is wired up and whether each channel is healthy before deciding where to route.

Both tools use the same daemon infrastructure that already exists (channel adapters, SharedDeps
dispatch, PendingAction confirmation). The work is additive — no existing behaviour changes.

## Requirements

### Req-1: `list_channels` Tool

`list_channels()` — List every configured channel with status.

- No input parameters.
- Iterates over the daemon's live channel registry (the same adapters used for inbound polling).
- For each channel, returns:
  - `name` — canonical identifier matching `Channel::name()` (e.g. `"telegram"`, `"discord"`,
    `"teams"`, `"imessage"`, `"email"`)
  - `connected` — boolean: has `connect()` succeeded and not subsequently errored?
  - `direction` — `"inbound"`, `"outbound"`, or `"bidirectional"` derived from channel
    capabilities (static per adapter type; see Req-5)
- Output: formatted table — one row per channel, columns: name | connected | direction
- Returns "No channels configured" if the channel registry is empty.
- This is a read-only tool. No PendingAction required.

### Req-2: `send_to_channel` Tool

`send_to_channel(channel, message)` — Send a message to a named channel.

Input parameters:
- `channel` (required, string) — matches `Channel::name()` for the target adapter (e.g.
  `"telegram"`, `"discord"`, `"teams"`). Case-insensitive match.
- `message` (required, string) — message body. Plain text; channel adapters handle formatting.

Behaviour:
1. Validate that a channel with the given name exists in the live registry. Return an error
   immediately if not found: `"Channel '<name>' not configured. Use list_channels to see
   available channels."`
2. Validate that the channel supports outbound sends (direction != "inbound"). Return an error
   if inbound-only: `"Channel '<name>' does not support outbound messages."`
3. Create a `PendingAction` with:
   - `description`: `"Send message to <channel>: <first 80 chars of message>…"`
   - `payload`: `{ "channel": "<name>", "message": "<full message>" }`
   - `status`: `AwaitingConfirmation`
4. Emit the confirmation prompt to Leo via the primary channel (Telegram) with the standard
   inline keyboard: `[Confirm] [Cancel]`.
5. On approval: call `channel.send_message(OutboundMessage { ... })` against the target channel
   adapter. Mark action `Executed`. Return `"Sent to <channel>."`.
6. On rejection: mark action `Rejected`. Return `"Cancelled — message not sent to <channel>."`.
7. On timeout (no response within 5 minutes): mark action `Expired`. Return
   `"Timed out — message not sent to <channel>."`.

### Req-3: Module Layout

New file: `crates/nv-daemon/src/tools/channels.rs`

Contains:
- `list_channels(registry: &ChannelRegistry) -> Result<String>` — sync-friendly, formats output.
- `send_to_channel(registry: &ChannelRegistry, channel: &str, message: &str) -> PendingActionRequest`
  — returns the pending action record for the caller to persist and confirm; does NOT call
  `send_message` directly (execution happens post-confirmation in the callback handler).
- `channels_tool_definitions() -> Vec<ToolDefinition>` — returns the two MCP tool definitions
  with full JSON Schema `inputSchema`.
- `ChannelInfo` struct (name, connected, direction) used for `list_channels` output.
- `ChannelDirection` enum (`Inbound`, `Outbound`, `Bidirectional`).

Add `pub mod channels;` to `crates/nv-daemon/src/tools/mod.rs`.

### Req-4: Daemon Integration — `SharedDeps`

The daemon's `SharedDeps` implementation must:
- Include `channels_tool_definitions()` in `daemon_tool_definitions()` return value.
- Handle `"list_channels"` and `"send_to_channel"` in its `call_tool()` dispatch.
- Pass a handle to the live channel registry into the tool dispatch path.

No changes to `nv-tools` stateless dispatch — these tools are daemon-coupled because they need
live channel handles.

### Req-5: Channel Direction Classification

| Channel | Direction |
|---------|-----------|
| `telegram` | Bidirectional |
| `discord` | Bidirectional |
| `teams` | Bidirectional |
| `imessage` | Bidirectional |
| `email` | Outbound (send-only in current implementation) |

Direction is a static property per adapter type. It does not require a live API probe.

### Req-6: `OutboundMessage` Construction

`send_to_channel` constructs an `OutboundMessage` using the message string as the body. The
`channel_id` / recipient is the channel's configured default target (e.g. for Telegram: the
configured chat ID; for Discord: the configured default guild/channel; for Teams: configured
team/channel). No per-call recipient override in this spec — routing is channel-level only.

### Req-7: Graceful Degradation

- If a channel is configured but not connected (e.g. bot token invalid), `send_to_channel`
  returns the error immediately after PendingAction approval rather than silently dropping.
- If `list_channels` cannot read connection state for an adapter, it marks that channel as
  `connected: false` rather than erroring out.
- If `send_to_channel` targets a channel that disconnected between confirmation and execution,
  it returns a user-facing error and marks the PendingAction as `Cancelled`.

### Req-8: System Prompt Update

Add `send_to_channel` and `list_channels` to the Reads/Writes tool lists in
`DEFAULT_SYSTEM_PROMPT` in `agent.rs`:
- `list_channels` → Reads (immediate, no confirm)
- `send_to_channel` → Writes (confirm first)

### Req-9: Audit Log

Both tool invocations are logged via the existing tool audit log:
- `list_channels`: log invocation + channel count returned.
- `send_to_channel`: log channel target, message length, PendingAction ID, and final status
  (executed / rejected / expired / error).

## Scope

**IN**: `channels.rs` tool module, `list_channels` tool (read-only), `send_to_channel` tool
(PendingAction-gated), `SharedDeps` registration, system prompt update, audit log entries,
`ChannelDirection` classification for all 5 adapters.

**OUT**: Per-recipient targeting within a channel (e.g. specific Discord user DM), channel
group/thread selection, message formatting per-channel (adapters handle their own formatting),
broadcast-to-all-channels, scheduling sends for a future time, reading messages via tools
(existing poll mechanism handles inbound), adding or removing channel configurations at runtime.

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/channels.rs` | New: `list_channels`, `send_to_channel`, `channels_tool_definitions`, `ChannelInfo`, `ChannelDirection` |
| `crates/nv-daemon/src/tools/mod.rs` | Add `pub mod channels;` |
| `crates/nv-daemon/src/worker.rs` (or SharedDeps impl) | Register and dispatch the two tools; pass channel registry |
| `crates/nv-daemon/src/agent.rs` | Add `list_channels` to Reads, `send_to_channel` to Writes in system prompt |
| No changes to `nv-tools/` | These tools are daemon-only — not added to stateless dispatch |

## Risks

| Risk | Mitigation |
|------|-----------|
| PendingAction callback not yet complete (depends on callback-handler-completion) | Block implementation on that Wave 3 spec. `send_to_channel` will not compile correctly without the callback resolution path. |
| Accidental cross-channel message delivery | PendingAction confirmation on every `send_to_channel` call, no exceptions. |
| Channel not connected at execution time (race) | Post-confirmation error with clear message; mark action `Cancelled`. |
| Message length limits per platform | Delegate to existing adapter `send_message` — adapters truncate/split as needed. Out of scope for this spec. |
| iMessage outbound support uncertain | If iMessage adapter does not implement outbound, classify as `Inbound` and return clear error on `send_to_channel` targeting it. |
