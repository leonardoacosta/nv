# Proposal: Add Discord CLI Tool

## Change ID
`add-discord-cli`

## Summary

Create a standalone TypeScript CLI (`discord-cli`) that wraps the Discord REST API for read operations, invokable by Nova via Bash. Surfaces guild listing, channel browsing, and message history through three subcommands using bot token auth.

## Context
- Extends: `packages/tools/discord-cli/` (new package, follows `packages/tools/teams-cli/` pattern)
- Related: `crates/nv-daemon/src/channels/discord/client.rs` (existing `DiscordRestClient` — Rust REST client patterns to mirror), archived `add-discord-read-tools` (same API surface, Rust daemon implementation), `add-teams-cli` (identical CLI scaffold pattern in TypeScript)
- Auth model: bot token (`DISCORD_BOT_TOKEN` from Doppler) via `Authorization: Bot {token}` header — no OAuth required
- Phase 2 — Tool Wrappers | Wave 4 (parallel to `add-teams-cli`, `add-ado-cli`)

## Motivation

Nova can read Discord messages via the daemon's `discord_read_messages` tool (gateway-connected), but that path requires the daemon process. A standalone CLI gives Nova a direct, process-independent path for Discord read operations via any Bash tool call — the same motivation that drove `teams-cli`. It also gives engineers a shell-accessible way to inspect Discord guild state without opening a browser or relying on the daemon being healthy.

## Requirements

### Req-1: CLI Entry Point and Command Structure

`src/index.ts` using commander with three subcommands:

- `guilds` — list all guilds (servers) the bot is a member of
- `channels <guild-id>` — list text channels in a guild, grouped by category
- `messages <channel-id> [--limit N]` — read recent messages from a channel

Default limit: 50. Max limit: 100.

#### Scenario: guilds subcommand
Running `discord-cli guilds` prints a formatted list of all guilds the bot is in, one per line, with guild ID and name.

#### Scenario: channels subcommand
Running `discord-cli channels 123456789` prints text channels for that guild grouped by their parent category. Channels with no category listed under `(uncategorized)`.

#### Scenario: messages subcommand
Running `discord-cli messages 987654321 --limit 20` prints the 20 most recent messages with author, relative timestamp, and content.

#### Scenario: missing guild-id
Running `discord-cli channels` without a guild ID exits 1 with usage hint.

### Req-2: Auth — Bot Token

`src/auth.ts` — `DiscordClient` class:

- Reads `DISCORD_BOT_TOKEN` from environment
- On missing env var: prints `Discord not configured — DISCORD_BOT_TOKEN env var not set` to stderr and exits 1
- Sets `Authorization: Bot {token}` header on all requests
- Exposes `get(path: string): Promise<unknown>` helper using built-in `fetch` (Node 18+)
- On HTTP 429: honor `Retry-After` header, wait, and retry once before failing

#### Scenario: missing token
Running any subcommand without `DISCORD_BOT_TOKEN` exits 1 with the "not configured" message.

#### Scenario: 401 Unauthorized
If the token is invalid, print `Discord auth failed — check DISCORD_BOT_TOKEN` and exit 1.

### Req-3: `guilds` Command

`src/commands/guilds.ts`

- Calls `GET /users/@me/guilds`
- Returns all guilds the bot is a member of
- Output format (one guild per line):
  ```
  Guilds (3)
  123456789012345678  Nova Homelab
  987654321098765432  Engineering Team
  111222333444555666  Personal Server
  ```

#### Scenario: bot in no guilds
Print `Bot is not a member of any guilds.` and exit 0.

### Req-4: `channels` Command

`src/commands/channels.ts`

- Calls `GET /guilds/{guild_id}/channels`
- Filters to text channels only (`type === 0`)
- Groups by `parent_id` — resolves category names from the same response (category channels have `type === 4`)
- Sorts channels within each category by `position`
- Output format:
  ```
  Channels — Nova Homelab (8 text channels)

  General
    general          987654321098765432
    announcements    876543210987654321

  Dev
    dev-chat         765432109876543210
    bot-testing      654321098765432109

  (uncategorized)
    random           543210987654321098
  ```

#### Scenario: guild not found
Discord returns 404 — print `Guild not found: {guild_id}` and exit 1.

#### Scenario: no text channels
Print `No text channels found in guild {guild_id}.` and exit 0.

### Req-5: `messages` Command

`src/commands/messages.ts`

- Calls `GET /channels/{channel_id}/messages?limit={limit}`
- Returns messages newest-first (Discord API default)
- Truncates message content to 500 chars, appending `…` if truncated
- Skips system messages (type !== 0)
- Output format:
  ```
  Messages — #general (last 20)
  [2h ago] nova_bot: Hello world
  [3h ago] leo: Can you check the latest build status?
  [5h ago] sarah: Deployment looks good
  ```

#### Scenario: channel not found
Discord returns 404 — print `Channel not found: {channel_id}` and exit 1.

#### Scenario: bot lacks read permission
Discord returns 403 — print `No permission to read channel {channel_id}` and exit 1.

#### Scenario: empty channel
Print `No messages found in channel {channel_id}.` and exit 0.

### Req-6: Output Format

All output to stdout as formatted plain text optimized for Claude readability. Timestamps as relative strings (e.g., "3h ago", "2d ago", "Mar 15"). No JSON output mode in v1. All errors to stderr.

### Req-7: Build and Install

`package.json` with:
- `build` script: `esbuild src/index.ts --bundle --platform=node --outfile=dist/discord-cli.js`
- `install-cli` script: copies `dist/discord-cli.js` to `~/.local/bin/discord-cli` with executable permission
- Shebang `#!/usr/bin/env node` prepended to dist output via build step

`tsconfig.json` targeting Node 20 with strict mode, `moduleResolution: bundler`.

Dependencies: `commander`, `esbuild` (dev). HTTP via built-in `fetch`. No additional runtime dependencies.

#### Scenario: build and install
Running `npm run build && npm run install-cli` produces `~/.local/bin/discord-cli` and `discord-cli guilds` executes successfully.

## Scope
- **IN**: `packages/tools/discord-cli/` package, three read subcommands (`guilds`, `channels`, `messages`), bot token auth, plain text output, esbuild bundle, install to `~/.local/bin`
- **OUT**: `nv-daemon` changes, send/write operations, slash command interaction, voice channels, thread messages, reaction handling, pagination beyond limit cap, JSON output flag, attachment content

## Impact
| Area | Change |
|------|--------|
| `packages/tools/discord-cli/` | New package — `src/index.ts`, `src/auth.ts`, `src/commands/guilds.ts`, `src/commands/channels.ts`, `src/commands/messages.ts`, `package.json`, `tsconfig.json` |
| `~/.local/bin/discord-cli` | Installed binary (not tracked in git) |
| Doppler | No new secrets — reuses existing `DISCORD_BOT_TOKEN` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Bot lacks `GUILD_MEMBERS` or `READ_MESSAGE_HISTORY` intent | REST API reads do not require gateway intents; bot must have channel read permissions in the Discord server settings |
| `packages/tools/` directory does not exist yet | `teams-cli` spec creates this directory; if applied in the same wave, apply `add-teams-cli` first or create `packages/tools/` as part of this spec |
| Node 18+ required for built-in `fetch` | Document in package README; homelab runs Node 20 |
| Rate limits on `GET /users/@me/guilds` | Low-frequency call; single retry on 429 with `Retry-After` header is sufficient |
