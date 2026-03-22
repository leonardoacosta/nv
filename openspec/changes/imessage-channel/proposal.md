# Proposal: iMessage Channel

## Change ID
`imessage-channel`

## Summary
Native iMessage channel adapter via BlueBubbles REST API. Polls for new messages on a configurable interval and sends replies through the BlueBubbles server running on a Mac. Implements the existing `Channel` trait from nv-core.

## Context
- Extends: `crates/nv-core/src/channel.rs` (Channel trait), `crates/nv-daemon/src/main.rs` (channel registration)
- Related: BlueBubbles server provides HTTP REST API for iMessage access from non-Apple devices; requires a Mac running BlueBubbles

## Motivation
Nova currently receives iMessage content only through manual relay. A native BlueBubbles adapter lets Nova poll for new iMessages directly, triage them alongside other channels, and respond — all without a relay bot. BlueBubbles exposes a simple HTTP API with password auth, making it the lowest-effort path to iMessage integration.

## Requirements

### Req-1: BlueBubbles API Types
Define Rust types with serde for BlueBubbles API responses: `BbMessage` (guid, text, date_created, handle, chat_guid, is_from_me), `BbChat` (guid, display_name, participants), `BbHandle` (address, service). These map directly to BlueBubbles REST endpoints.

### Req-2: BlueBubblesClient
HTTP client (reqwest) wrapping the BlueBubbles REST API. Methods:
- `get_messages(after: i64, limit: u32)` — GET `/api/v1/message` with `after` timestamp filter
- `send_message(chat_guid: &str, text: &str)` — POST `/api/v1/message/text`
- All requests include the `password` query parameter for auth

### Req-3: IMessageChannel (Channel Trait)
Implement the `Channel` trait for `IMessageChannel`. On each poll tick, fetch new messages since the last seen timestamp, convert to `InboundMessage`, and emit onto the trigger mpsc channel. Sending replies calls `send_message` on the BlueBubbles client.

### Req-4: Polling Loop
A tokio task polls BlueBubbles on a configurable interval (`imessage_poll_interval_secs`, default: 10). Tracks the last seen message timestamp to avoid duplicates. Backs off on consecutive errors (double interval, cap at 5 minutes).

### Req-5: Configuration
New `[imessage]` section in `nv.toml`:
- `enabled` (bool, default: false)
- `bluebubbles_url` (String, e.g. "http://mac.tailnet:1234")
- `bluebubbles_password` (String)
- `poll_interval_secs` (u64, default: 10)

## Scope
- **IN**: BlueBubbles API types, HTTP client, Channel trait implementation, polling loop, config section, daemon wiring, unit tests
- **OUT**: BlueBubbles webhook mode (polling only for simplicity), group chat handling (DMs only initially), media/attachment forwarding, BlueBubbles server setup/provisioning

## Impact
| Area | Change |
|------|--------|
| crates/nv-daemon/src/imessage/mod.rs | New module: BlueBubblesClient, API types |
| crates/nv-daemon/src/imessage/channel.rs | IMessageChannel implementing Channel trait |
| crates/nv-core/src/config.rs | Add IMessageConfig to DaemonConfig |
| crates/nv-daemon/src/main.rs | Spawn iMessage polling task if enabled |

## Risks
| Risk | Mitigation |
|------|-----------|
| BlueBubbles server offline (Mac sleeping) | Log warning on connection failure, continue polling — back off on errors |
| BlueBubbles API changes between versions | Pin to v1 API, types are minimal and stable |
| Polling latency (up to poll_interval delay) | Default 10s is acceptable; user can tune down to 2s |
| Password in config file | Same pattern as other secrets in nv.toml; env file is gitignored |
