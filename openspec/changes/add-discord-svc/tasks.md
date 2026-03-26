# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch: Package + Client

- [x] [1.1] [P-1] Add `packages/tools/*` to `pnpm-workspace.yaml` packages glob (enables discord-svc, discord-cli, teams-cli as workspace packages) [owner:api-engineer]
- [x] [1.2] [P-1] Create `packages/tools/discord-svc/package.json` — name `@nova/discord-svc`, dependencies: `hono`, `@hono/node-server`, `pino`; devDependencies: `@types/node`, `typescript`, `pino-pretty`; scripts: `dev` (tsx --watch src/index.ts), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit) [owner:api-engineer]
- [x] [1.3] [P-1] Create `packages/tools/discord-svc/tsconfig.json` — strict, ES2022, NodeNext module/moduleResolution, outDir dist, rootDir src, declaration true, sourceMap true, skipLibCheck true — match daemon tsconfig pattern [owner:api-engineer]
- [x] [1.4] [P-1] Create `packages/tools/discord-svc/src/client.ts` — `DiscordClient` class wrapping Discord REST API v10. Constructor takes token string. `async get(path)` method with `Authorization: Bot {token}` header, 429 retry-once with Retry-After, throw on 401/403/404 with descriptive messages (not process.exit). Port from `packages/tools/discord-cli/src/auth.ts` but throw instead of exit. [owner:api-engineer]
- [x] [1.5] [P-1] Create `packages/tools/discord-svc/src/logger.ts` — `createLogger(name)` using pino with pino-pretty in dev mode, match daemon logger pattern from `packages/daemon/src/logger.ts` [owner:api-engineer]

## API Batch: Tool Handlers

- [x] [2.1] [P-1] Create `packages/tools/discord-svc/src/tools/guilds.ts` — `listGuilds(client)` calling `GET /users/@me/guilds`, returns `{ guilds: Array<{id, name, icon}> }`. Port from `packages/tools/discord-cli/src/commands/guilds.ts` but return structured data instead of console.log. [owner:api-engineer]
- [x] [2.2] [P-1] Create `packages/tools/discord-svc/src/tools/channels.ts` — `listChannels(client, guildId)` calling `GET /guilds/{guildId}/channels`, filter type 0 text channels, resolve category names from type 4, group by category, sort by position. Return `{ guild_id, channels: Array<{id, name, category, position}> }`. Port from `packages/tools/discord-cli/src/commands/channels.ts`. [owner:api-engineer]
- [x] [2.3] [P-1] Create `packages/tools/discord-svc/src/tools/messages.ts` — `readMessages(client, channelId, limit)` calling `GET /channels/{channelId}/messages?limit={limit}`, filter system messages (type !== 0), truncate content to 500 chars, use global_name with username fallback. Return `{ channel_id, messages: Array<{id, author, content, timestamp}> }`. Port from `packages/tools/discord-cli/src/commands/messages.ts`. [owner:api-engineer]

## API Batch: HTTP Server + MCP

- [x] [3.1] [P-1] Create `packages/tools/discord-svc/src/index.ts` — Hono app entry point. Read `DISCORD_BOT_TOKEN` from env (exit 1 if missing). Mount routes: `GET /health`, `GET /guilds`, `GET /channels/:guildId`, `GET /messages/:channelId`. Serve on port 4004 via `@hono/node-server`. SIGTERM/SIGINT graceful shutdown. Check for `--mcp` flag and branch to MCP mode. [owner:api-engineer]
- [x] [3.2] [P-1] Implement `GET /health` route — return `{ status: "ok", service: "discord-svc", port: 4004 }` with 200 [owner:api-engineer]
- [x] [3.3] [P-1] Implement `GET /guilds` route — call `listGuilds(client)`, return JSON result, catch errors and return appropriate HTTP status (401/403/500) with `{ error: message }` [owner:api-engineer]
- [x] [3.4] [P-1] Implement `GET /channels/:guildId` route — call `listChannels(client, guildId)`, 404 on guild not found, 403 on permission denied [owner:api-engineer]
- [x] [3.5] [P-1] Implement `GET /messages/:channelId` route — parse `?limit=N` query param (default 50, clamp to 1-100), call `readMessages(client, channelId, limit)`, 404 on channel not found, 403 on permission denied [owner:api-engineer]
- [x] [3.6] [P-2] Create `packages/tools/discord-svc/src/mcp.ts` — minimal MCP stdio server implementing JSON-RPC `initialize`, `tools/list`, `tools/call` methods. Register 3 tool definitions matching Rust tool schemas from `crates/nv-daemon/src/tools/discord.rs`. Read stdin line-by-line, parse JSON-RPC, dispatch to tool handlers, write JSON-RPC response to stdout. [owner:api-engineer]

## Verify

- [x] [4.1] `pnpm install` succeeds with new workspace package [owner:api-engineer]
- [x] [4.2] `cd packages/tools/discord-svc && pnpm typecheck` passes with zero errors [owner:api-engineer]
- [x] [4.3] `cd packages/tools/discord-svc && pnpm build` produces dist/ output [owner:api-engineer]
- [x] [4.4] Unit test: DiscordClient throws on missing token, throws descriptive errors for 401/403/404 [owner:api-engineer]
- [x] [4.5] Unit test: listChannels groups by category, sorts by position, filters to text only [owner:api-engineer]
- [x] [4.6] Unit test: readMessages truncates content at 500 chars, filters system messages, uses global_name fallback [owner:api-engineer]
- [x] [4.7] Unit test: health endpoint returns correct JSON structure [owner:api-engineer]
- [x] [4.8] Unit test: MCP tools/list returns 3 tool definitions with correct names and schemas [owner:api-engineer]
- [ ] [4.9] [user] Manual test: start service with `DISCORD_BOT_TOKEN`, `curl localhost:4004/health` returns ok [owner:api-engineer]
- [ ] [4.10] [user] Manual test: `curl localhost:4004/guilds` returns bot's guilds [owner:api-engineer]
