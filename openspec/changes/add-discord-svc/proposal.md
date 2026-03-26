# Proposal: Add Discord Service

## Change ID
`add-discord-svc`

## Summary

Create the Discord tool service (`discord-svc`) at port 4004 as a Hono microservice wrapping the Discord Bot REST API. Exposes three read-only tools — `discord_list_guilds`, `discord_list_channels`, `discord_read_messages` — via HTTP routes and MCP stdio transport. Ports logic from the existing `packages/tools/discord-cli/` and `crates/nv-daemon/src/tools/discord.rs`.

## Context

- Extends: `packages/tools/discord-svc/` (new service, follows the scaffold-tool-service template pattern established by other `-svc` packages in v10)
- Related: `packages/tools/discord-cli/` (existing CLI tool — port the `DiscordClient` class and command logic), `crates/nv-daemon/src/tools/discord.rs` (Rust implementation with same 3 tools), `crates/nv-daemon/src/channels/discord/client.rs` (Rust REST client)
- Dependencies: `scaffold-tool-service` (Wave 1 — service template pattern), `add-channels-svc` (Wave 3 — cross-channel dispatch; discord-svc is a channel adapter but does not call channels-svc at runtime in v1)
- Auth: `DISCORD_BOT_TOKEN` from Doppler, `Authorization: Bot {token}` header
- Phase 3 — Communication Tools | Wave 4

## Motivation

Nova's Discord read capability currently exists in two places: the Rust daemon tools (being decommissioned in v10) and a standalone CLI. Neither fits the v10 tool fleet architecture where each domain runs as an independent Hono service with dual HTTP+MCP transport. A dedicated `discord-svc` at port 4004 gives the agent native MCP tool discovery for Discord reads, gives the dashboard direct HTTP access for rendering Discord data, and maintains fault isolation from other tool services.

## Requirements

### Req-1: Service Entry Point

`src/index.ts` — Hono app on port 4004 with pino logger. Reads `DISCORD_BOT_TOKEN` from env on startup; exits 1 with clear error if missing. Graceful shutdown on SIGTERM/SIGINT.

#### Scenario: missing token
Starting without `DISCORD_BOT_TOKEN` logs `DISCORD_BOT_TOKEN not set — exiting` and exits 1.

#### Scenario: startup
Starting with valid env logs `discord-svc listening on :4004` and responds to health checks.

### Req-2: Discord API Client

`src/client.ts` — `DiscordClient` class wrapping `https://discord.com/api/v10`.

- Constructor takes bot token string
- `get(path: string): Promise<unknown>` — sets `Authorization: Bot {token}`, handles:
  - HTTP 429: honor `Retry-After` header, sleep, retry once
  - HTTP 401: throw with "Discord auth failed" message
  - HTTP 403: throw with "No permission" + resource context
  - HTTP 404: throw with "Not found" + resource context
- Reuse the exact error handling pattern from `packages/tools/discord-cli/src/auth.ts` but throw errors instead of `process.exit` (service stays alive on bad requests)

### Req-3: discord_list_guilds Tool

`src/tools/guilds.ts`

- Calls `GET /users/@me/guilds`
- Returns `{ guilds: Array<{ id: string; name: string; icon: string | null }> }`
- Empty result returns `{ guilds: [] }`

HTTP route: `GET /guilds`

### Req-4: discord_list_channels Tool

`src/tools/channels.ts`

- Calls `GET /guilds/{guild_id}/channels`
- Filters to text channels (type 0), resolves category names from type 4 channels
- Groups by category, sorts by position within each group
- Returns `{ guild_id: string; channels: Array<{ id: string; name: string; category: string; position: number }> }`

HTTP route: `GET /channels/:guildId`

#### Scenario: guild not found
Discord 404 response returns HTTP 404 with `{ error: "Guild not found: {guild_id}" }`

### Req-5: discord_read_messages Tool

`src/tools/messages.ts`

- Calls `GET /channels/{channel_id}/messages?limit={limit}`
- Default limit: 50, max: 100
- Filters out system messages (type !== 0)
- Truncates content to 500 chars with `...` suffix
- Returns `{ channel_id: string; messages: Array<{ id: string; author: string; content: string; timestamp: string }> }`
- Author uses `global_name` with fallback to `username`

HTTP route: `GET /messages/:channelId?limit=N`

#### Scenario: no permission
Discord 403 response returns HTTP 403 with `{ error: "No permission to read channel {channel_id}" }`

#### Scenario: empty channel
Returns `{ channel_id, messages: [] }`

### Req-6: Health Endpoint

`GET /health` returns `{ status: "ok", service: "discord-svc", port: 4004 }` (200).

If the bot token is present but untested, still return ok (token validation happens on first API call, not on health check).

### Req-7: MCP Server (stdio)

`src/mcp.ts` — MCP stdio transport exposing 3 tool definitions:

- `discord_list_guilds` — no params
- `discord_list_channels` — `{ guild_id: string }` required
- `discord_read_messages` — `{ channel_id: string }` required, `{ limit: number }` optional

MCP tool descriptions match the Rust definitions in `crates/nv-daemon/src/tools/discord.rs` (lines 196-248).

The service binary supports two modes:
- `node dist/index.js` — starts HTTP server (default)
- `node dist/index.js --mcp` — starts MCP stdio server

### Req-8: Package Setup

`packages/tools/discord-svc/package.json`:
- Name: `@nova/discord-svc`
- Dependencies: `hono`, `@hono/node-server`, `pino`
- Dev: `@types/node`, `typescript`, `pino-pretty`
- Scripts: `dev` (tsx --watch), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit)

`tsconfig.json`: strict, ES2022, NodeNext module resolution — match daemon pattern.

No esbuild bundle — this is a long-running service, not a CLI binary.

## Scope

- **IN**: `packages/tools/discord-svc/` — Hono HTTP server on :4004, MCP stdio server, 3 read-only Discord tools, health endpoint, pino logging, bot token auth
- **OUT**: Write operations (sending messages — channels-svc handles outbound), Discord gateway/WebSocket, slash commands, voice channels, thread messages, reactions, file attachment content, systemd unit file (add-fleet-deploy handles that), Traefik config (separate spec), dashboard integration

## Impact

| Area | Change |
|------|--------|
| `packages/tools/discord-svc/` | New package — `src/index.ts`, `src/client.ts`, `src/tools/guilds.ts`, `src/tools/channels.ts`, `src/tools/messages.ts`, `src/mcp.ts`, `package.json`, `tsconfig.json` |
| `pnpm-workspace.yaml` | Add `packages/tools/*` glob (if not already present) |
| Doppler | No new secrets — reuses existing `DISCORD_BOT_TOKEN` |

## Risks

| Risk | Mitigation |
|------|-----------|
| scaffold-tool-service not yet implemented | discord-svc is self-contained; follows daemon's Hono pattern directly. No template dependency at runtime — only pattern alignment. |
| pnpm-workspace.yaml does not include `packages/tools/*` | Add the glob to workspace config as part of this spec; it also benefits discord-cli, teams-cli |
| MCP SDK may not exist for Node yet | Implement minimal JSON-RPC stdio protocol (initialize, tools/list, tools/call) — same 3-method subset used in nv-tools Rust crate |
| Rate limits on Discord API | Single retry on 429 with Retry-After header; service stays alive on transient errors |
| Bot lacks permissions in some guilds/channels | Return structured error responses (403/404) instead of crashing; client handles gracefully |
