# Implementation Tasks

<!-- beads:epic:nv-98mn -->

## API Batch

- [x] [1.1] [P-1] Add `DiscordGuild`, `DiscordChannel`, `DiscordMessage`, `DiscordAuthor` structs to `crates/nv-daemon/src/channels/discord/types.rs` — all derive `Deserialize, Debug, Clone`, use `#[serde(rename_all = "snake_case")]` where needed [owner:api-engineer]
- [x] [1.2] [P-1] Add `pub async fn list_guilds(&self) -> Result<Vec<DiscordGuild>>` to `DiscordRestClient` — `GET /users/@me/guilds` with Bot auth header [owner:api-engineer]
- [x] [1.3] [P-1] Add `pub async fn list_channels(&self, guild_id: &str) -> Result<Vec<DiscordChannel>>` to `DiscordRestClient` — `GET /guilds/{guild_id}/channels`, filter to type 0 (text), sort by position [owner:api-engineer]
- [x] [1.4] [P-1] Add `pub async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<DiscordMessage>>` to `DiscordRestClient` — `GET /channels/{channel_id}/messages?limit={limit}` [owner:api-engineer]
- [x] [1.5] [P-1] Extract `async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response>` private helper in DiscordRestClient — reuse the existing 429 rate-limit retry logic from post_message [owner:api-engineer]
- [x] [1.6] [P-2] Create `crates/nv-daemon/src/tools/discord.rs` — `discord_list_guilds`, `discord_list_channels`, `discord_read_messages` tool functions + `discord_tool_definitions()` returning 3 ToolDefinition entries [owner:api-engineer]
- [x] [1.7] [P-2] Wire dispatch for all 3 discord tools in `tools/mod.rs` — match tool names, build DiscordRestClient from bot token in secrets, call functions, return ToolResult::Immediate [owner:api-engineer]
- [x] [1.8] [P-3] Update `config/system-prompt.md` — add `discord_list_guilds`, `discord_list_channels`, `discord_read_messages` to "Reads (immediate)" tools list [owner:api-engineer]

## Verify

- [x] [2.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [x] [2.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [x] [2.3] Unit test: discord_list_channels formats text channels grouped by category [owner:api-engineer]
- [x] [2.4] Unit test: discord_read_messages truncates long content to 500 chars [owner:api-engineer]
- [x] [2.5] Unit test: tool definitions include all 3 new discord tools [owner:api-engineer]
- [ ] [2.6] [user] Manual test: ask Nova "list my Discord servers" — verify guilds appear [owner:api-engineer]
- [ ] [2.7] [user] Manual test: ask Nova "read #general in [server]" — verify messages render [owner:api-engineer]
