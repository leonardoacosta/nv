# Proposal: Add Discord Read Tools

## Change ID
`add-discord-read-tools`

## Summary

Add three Discord tools — `discord_list_channels`, `discord_read_messages`, and `discord_list_guilds` — so Nova can proactively read Discord server history on demand. The Discord bot is already connected via gateway and can send messages; this adds read-back capability.

## Context
- Extends: `crates/nv-daemon/src/channels/discord/client.rs` (DiscordRestClient), `crates/nv-daemon/src/tools/` (no discord tools exist yet)
- Related: Discord gateway adapter already handles inbound messages; DiscordRestClient has send_message with rate-limit retry
- Bot token already configured via DISCORD_BOT_TOKEN in Doppler

## Motivation

Nova receives Discord messages via the gateway WebSocket and can reply, but cannot proactively read message history or browse channels. When Leo asks "what's the latest in #general" or "catch me up on Discord", Nova has no tool to fetch that context. This is the same gap that existed for Teams DMs (now fixed) — the channel adapter works bidirectionally but no read tools are registered.

## Requirements

### Req-1: DiscordRestClient — list_guilds

Add `pub async fn list_guilds(&self) -> Result<Vec<DiscordGuild>>` to DiscordRestClient.

- Calls `GET /users/@me/guilds`
- Returns servers the bot is in with id, name, icon

### Req-2: DiscordRestClient — list_channels

Add `pub async fn list_channels(&self, guild_id: &str) -> Result<Vec<DiscordChannel>>` to DiscordRestClient.

- Calls `GET /guilds/{guild_id}/channels`
- Returns text channels only (type 0), sorted by position
- Each channel has id, name, topic, parent_id (category)

### Req-3: DiscordRestClient — get_messages

Add `pub async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<DiscordMessage>>` to DiscordRestClient.

- Calls `GET /channels/{channel_id}/messages?limit={limit}`
- Returns messages newest-first, limit default 20, max 50
- DiscordMessage: id, content, author (username, display_name), timestamp, attachments count

### Req-4: discord_list_guilds tool

- Input: none
- Output: formatted list of servers the bot is in

### Req-5: discord_list_channels tool

- Input: `{ "guild_id": "required" }` (or guild name, resolved from list_guilds)
- Output: formatted channel list grouped by category

### Req-6: discord_read_messages tool

- Input: `{ "channel_id": "required", "limit": 20 }`
- Output: formatted message list — author, timestamp, content (truncated 500 chars)

### Req-7: System prompt update

Add all three tools to "Reads (immediate)" in config/system-prompt.md.

## Scope
- **IN**: 3 client methods, 3 tool definitions, dispatch wiring, system prompt update, unit tests
- **OUT**: Discord slash commands, reactions, threads, voice channels, file attachments content

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/discord/client.rs` | Add list_guilds, list_channels, get_messages methods |
| `crates/nv-daemon/src/channels/discord/types.rs` | Add DiscordGuild, DiscordChannel, DiscordMessage REST types |
| `crates/nv-daemon/src/tools/discord.rs` | New module — 3 tool functions + definitions |
| `crates/nv-daemon/src/tools/mod.rs` | Wire dispatch for 3 new tools |
| `config/system-prompt.md` | Add tool names to reads list |

## Risks

| Risk | Mitigation |
|------|-----------|
| Bot may lack permissions in some channels | Discord returns 403 for forbidden channels — handle gracefully |
| Message content may be empty (embeds-only) | Show "[embed]" placeholder for messages with no text content |
| Rate limits on Discord API | Reuse existing retry pattern from send_message |
