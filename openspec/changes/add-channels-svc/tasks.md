# Implementation Tasks

## Setup Batch

- [ ] [1.1] [P-1] Create `packages/tools/channels-svc/package.json` -- name `@nova/channels-svc`, private, type module, scripts: `dev` (tsx --watch), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit), `test` (node --import tsx/esm --test tests/**/*.test.ts). Dependencies: hono, @hono/node-server, pino. Dev deps: @types/node, tsx, typescript, pino-pretty [owner:api-engineer]
- [ ] [1.2] [P-1] Create `packages/tools/channels-svc/tsconfig.json` -- target ES2022, module NodeNext, moduleResolution NodeNext, outDir dist, rootDir src, strict true, declaration true, composite true. Match daemon's tsconfig patterns [owner:api-engineer]
- [ ] [1.3] [P-1] Run `pnpm install` from workspace root to link the new package [owner:api-engineer]

## Types Batch

- [ ] [2.1] [P-1] Create `src/types.ts` -- export `ChannelName` (union of 5 channel strings), `ChannelStatus` (connected | disconnected | error), `ChannelDirection` (inbound | outbound | bidirectional), `ChannelInfo` (name + status + direction), `SendRequest` (channel + target + message), `SendResult` (ok boolean + optional error) [owner:api-engineer]
- [ ] [2.2] [P-1] Create `src/logger.ts` -- export default pino instance with `name: "channels-svc"` [owner:api-engineer]

## Registry Batch

- [ ] [3.1] [P-1] Create `src/adapters/registry.ts` -- define `ChannelAdapter` interface with `name`, `direction`, `status()`, `send(target, message)`. Implement `AdapterRegistry` class with `register()`, `get()`, `list()`, `has()` methods. Map stores adapters by `ChannelName` [owner:api-engineer]
- [ ] [3.2] [P-1] Create `src/adapters/telegram.ts` -- implement `ChannelAdapter` for Telegram. Constructor takes `botToken` from env. `send(chatId, message)` calls Telegram Bot API `sendMessage` via `fetch()` with plain text (no parse_mode). `status()` returns `"connected"` if token present, `"disconnected"` otherwise. Direction: `"bidirectional"` [owner:api-engineer]
- [ ] [3.3] [P-2] Create `src/adapters/discord.ts` -- stub adapter. Direction bidirectional, status disconnected, send throws "Discord adapter not yet implemented" [owner:api-engineer]
- [ ] [3.4] [P-2] Create `src/adapters/teams.ts` -- stub adapter. Direction bidirectional, status disconnected, send throws "Teams adapter not yet implemented" [owner:api-engineer]
- [ ] [3.5] [P-2] Create `src/adapters/email.ts` -- stub adapter. Direction outbound, status disconnected, send throws "Email adapter not yet implemented" [owner:api-engineer]
- [ ] [3.6] [P-2] Create `src/adapters/imessage.ts` -- stub adapter. Direction bidirectional, status disconnected, send throws "iMessage adapter not yet implemented" [owner:api-engineer]

## HTTP Batch

- [ ] [4.1] [P-1] Create `src/server.ts` -- Hono app with three routes: `GET /health` returns service info, `GET /channels` returns adapter registry list, `POST /send` validates + dispatches via registry. Validation: channel must exist (404), must support outbound (400), must be connected (503). On adapter error return 502 [owner:api-engineer]
- [ ] [4.2] [P-1] Create `src/index.ts` -- entry point. Instantiate `AdapterRegistry`, register Telegram adapter (from `TELEGRAM_BOT_TOKEN` env), register stub adapters. Start Hono server on `PORT` env (default 4003). Log startup with port and registered adapter count. If `--mcp` flag or stdin is piped, also start MCP stdio handler [owner:api-engineer]

## MCP Batch

- [ ] [5.1] [P-2] Create `src/mcp.ts` -- MCP stdio JSON-RPC handler. Reads newline-delimited JSON from stdin, handles `initialize` (return server info), `tools/list` (return list_channels + send_to_channel definitions), `tools/call` (dispatch to adapter registry). Writes JSON responses to stdout. Uses same `AdapterRegistry` instance as HTTP server [owner:api-engineer]

## Test Batch

- [ ] [6.1] [P-1] Create `tests/registry.test.ts` -- unit tests for `AdapterRegistry`: register adapter, list returns it, get by name, has returns true/false, list returns all registered adapters sorted, register overwrites existing [owner:api-engineer]
- [ ] [6.2] [P-1] Create `tests/server.test.ts` -- HTTP route tests using Hono test client: `GET /health` returns 200 with service name, `GET /channels` returns registered channels, `POST /send` with valid connected channel returns ok, `POST /send` with unknown channel returns 404, `POST /send` with disconnected channel returns 503 [owner:api-engineer]

## Verify

- [ ] [7.1] `tsc --noEmit` passes in channels-svc [owner:api-engineer]
- [ ] [7.2] `node --import tsx/esm --test tests/**/*.test.ts` -- all tests pass [owner:api-engineer]
- [ ] [user] Manual: start service with `TELEGRAM_BOT_TOKEN` set, `curl localhost:4003/channels` shows telegram as connected
- [ ] [user] Manual: `curl -X POST localhost:4003/send -H 'Content-Type: application/json' -d '{"channel":"telegram","target":"<chat_id>","message":"test"}'` delivers message
