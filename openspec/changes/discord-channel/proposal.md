# Proposal: Discord Channel

## Change ID
`discord-channel`

## Summary

Native Discord gateway WebSocket + REST adapter replacing the Python relay bot. Connects to
Discord's gateway for real-time message events, sends replies via REST API, and implements the
existing `Channel` trait for seamless integration with the agent loop.

## Context
- Extends: `crates/nv-core/src/channel.rs` (implements `Channel` trait)
- Related: `crates/nv-daemon/src/telegram/` (reference implementation of a Channel adapter), `crates/nv-core/src/config.rs` (config structs)

## Motivation

Nova currently receives Discord messages via a Python relay bot that forwards to Telegram. This
adds latency, a Python runtime dependency, and a single point of failure. A native Rust adapter
eliminates the relay, reduces latency, and brings Discord on par with Telegram as a first-class
channel.

Benefits:
1. **Direct integration** -- no relay bot, no Python dependency
2. **Lower latency** -- WebSocket gateway delivers events in real time
3. **Reliability** -- one fewer moving part in the pipeline
4. **Channel trait** -- plugs directly into the agent loop via `poll_messages()` / `send_message()`

## Requirements

### Req-1: Discord API Types

Define Rust types for Discord API objects needed: Guild, Channel, Message, User, GatewayPayload,
GatewayEvent (with serde Serialize/Deserialize). Only model the fields Nova actually uses --
Discord API objects are large, keep it minimal.

### Req-2: Gateway WebSocket Connection

Connect to the Discord gateway (`wss://gateway.discord.gg`) using tokio-tungstenite. Handle the
gateway lifecycle: Identify (with bot token + intents), Heartbeat (periodic ACK), Resume (on
reconnect), and Dispatch (MESSAGE_CREATE, READY, etc.). Buffer incoming MESSAGE_CREATE events
for `poll_messages()`.

### Req-3: REST API Client

HTTP client (reqwest) for sending messages via `POST /channels/{id}/messages`. Handle rate
limiting (429 responses with Retry-After header). Support message content up to Discord's 2000
character limit with automatic chunking for longer responses.

### Req-4: Channel Trait Implementation

`DiscordChannel` implements the `Channel` trait:
- `name()` returns `"discord"`
- `connect()` establishes the gateway WebSocket and authenticates
- `poll_messages()` drains buffered MESSAGE_CREATE events, converts to `InboundMessage`
- `send_message()` sends via REST API to the appropriate channel
- `disconnect()` sends close frame and drops the WebSocket

### Req-5: Configuration

Add `[discord]` section to `nv.toml`:
- `server_ids`: Vec of guild IDs to watch
- `channel_ids`: Vec of channel IDs to watch (messages from unwatched channels are dropped)

Add `DISCORD_BOT_TOKEN` to `Secrets` struct (read from environment variable).

### Req-6: Message Filtering

Only process messages from configured `channel_ids` within configured `server_ids`. Ignore
messages from the bot itself (prevent echo loops). Optionally handle DMs and @mentions.

### Req-7: Daemon Wiring

Register `DiscordChannel` in the daemon's channel map (alongside Telegram). Spawn as a tokio
task. Inject into agent loop trigger channel via mpsc.

## Scope
- **IN**: Gateway WebSocket, REST send, Discord types (minimal), Channel trait impl, config, filtering, daemon wiring
- **OUT**: Slash commands, reactions, embeds (rich formatting), voice channels, file attachments, presence/status updates

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/discord/mod.rs` | New module: DiscordChannel implementing Channel trait |
| `crates/nv-daemon/src/discord/types.rs` | New: Discord API types with serde |
| `crates/nv-daemon/src/discord/gateway.rs` | New: WebSocket gateway connection + event handling |
| `crates/nv-daemon/src/discord/client.rs` | New: REST API client for sending messages |
| `crates/nv-core/src/config.rs` | Add DiscordConfig struct, add `discord: Option<DiscordConfig>` to Config |
| `crates/nv-core/src/config.rs` | Add `discord_bot_token: Option<String>` to Secrets |
| `crates/nv-daemon/src/main.rs` | Conditionally spawn DiscordChannel, register in channel map |

## Risks
| Risk | Mitigation |
|------|-----------|
| Gateway disconnects (network, Discord outages) | Auto-resume with session ID; exponential backoff reconnect |
| Rate limiting on REST sends | Respect 429 + Retry-After; queue outbound messages |
| Gateway intents require privileged access (Message Content) | Document bot setup: enable Message Content intent in Discord Developer Portal |
| tokio-tungstenite version conflicts with other deps | Pin compatible version in Cargo.toml |
