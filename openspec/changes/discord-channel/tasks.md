# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Add DiscordConfig to nv-core/config.rs: server_ids (Vec<u64>), channel_ids (Vec<u64>); add `discord: Option<DiscordConfig>` to Config struct [owner:api-engineer]
- [ ] [2.2] [P-1] Add discord_bot_token (Option<String>) to Secrets struct, read from DISCORD_BOT_TOKEN env var [owner:api-engineer]
- [ ] [2.3] [P-1] Create crates/nv-daemon/src/discord/types.rs — minimal Discord API types with serde: GatewayPayload, GatewayEvent (Identify, Heartbeat, HeartbeatAck, Ready, Resumed, Dispatch), Message, User, Channel (id, name fields only) [owner:api-engineer]
- [ ] [2.4] [P-1] Create crates/nv-daemon/src/discord/gateway.rs — WebSocket connection to wss://gateway.discord.gg/?v=10&encoding=json using tokio-tungstenite; handle Identify (bot token + GUILD_MESSAGES + MESSAGE_CONTENT intents), Heartbeat loop (interval from Hello), Resume on reconnect [owner:api-engineer]
- [ ] [2.5] [P-1] Add MESSAGE_CREATE dispatch handling in gateway.rs — parse into InboundMessage, filter by configured channel_ids/server_ids, ignore self-messages (bot user ID from Ready), buffer in Arc<Mutex<VecDeque>> [owner:api-engineer]
- [ ] [2.6] [P-1] Create crates/nv-daemon/src/discord/client.rs — REST client (reqwest) for POST /channels/{id}/messages; handle 429 rate limits (Retry-After header, sleep + retry); auto-chunk messages >2000 chars [owner:api-engineer]
- [ ] [2.7] [P-1] Create crates/nv-daemon/src/discord/mod.rs — DiscordChannel struct implementing Channel trait: name() -> "discord", connect() starts gateway task, poll_messages() drains buffered events, send_message() calls REST client, disconnect() sends close frame [owner:api-engineer]
- [ ] [2.8] [P-2] Wire DiscordChannel into main.rs — conditionally spawn when discord config + token present, register in channel map, connect to mpsc trigger channel [owner:api-engineer]
- [ ] [2.9] [P-2] Add mod discord declaration in main.rs or lib.rs [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit tests: gateway payload serialization/deserialization (Identify, Heartbeat, Dispatch) [owner:api-engineer]
- [ ] [3.4] Unit tests: message filtering (accepted channel, rejected channel, self-message ignored) [owner:api-engineer]
- [ ] [3.5] Unit tests: REST client message chunking (under limit, at limit, over limit) [owner:api-engineer]
- [ ] [3.6] Unit tests: InboundMessage conversion from Discord Message [owner:api-engineer]
- [ ] [3.7] Existing tests pass [owner:api-engineer]
