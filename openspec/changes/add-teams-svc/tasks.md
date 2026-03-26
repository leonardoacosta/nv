# Implementation Tasks

## Phase 1: Package Scaffold and SSH Helper

- [ ] [1.1] Create `packages/tools/teams-svc/package.json` (`@nova/teams-svc`), `tsconfig.json` (strict, Node 20, ESM), and `build.mjs` (esbuild bundler). Scripts: `build` (esbuild src/index.ts -> dist/index.js), `dev` (tsx watch src/index.ts), `start` (node dist/index.js), `typecheck` (tsc --noEmit). Dependencies: `hono`, `pino`. Dev dependencies: `@types/node`, `esbuild`, `tsx`, `typescript`. [owner:api-engineer]
- [ ] [1.2] Create `src/ssh.ts` — export `sshCloudPc(script: string, args: string): Promise<string>`. Spawn `ssh -o ConnectTimeout=10 cloudpc "powershell -ExecutionPolicy Bypass -Command \"& { . C:\Users\leo.346-CPC-QJXVZ\{script} {args} }\""`. Capture stdout, filter noise lines (WARNING, vulnerable, upgraded, security fix). Throw "CloudPC unreachable" on connection failures (Connection refused, timed out, No route to host). Throw with stderr on non-zero exit. Port directly from `crates/nv-daemon/src/tools/cloudpc.rs` logic. [owner:api-engineer]

## Phase 2: Tool Handlers

- [ ] [2.1] Create `src/tools/list-chats.ts` — `teamsListChats(limit?: number): Promise<string>`. Clamp limit to 1-50 (default 20). Call `sshCloudPc("graph-teams.ps1", "-Action list")`. Port from `teams_list_chats` in `crates/nv-daemon/src/tools/teams.rs`. [owner:api-engineer]
- [ ] [2.2] Create `src/tools/read-chat.ts` — `teamsReadChat(chatId: string, limit?: number): Promise<string>`. Validate chatId non-empty. Clamp limit 1-50 (default 20). Call `sshCloudPc("graph-teams.ps1", "-Action messages -ChatId '{chatId}' -Count {limit}")`. Port from `teams_read_chat` in `crates/nv-daemon/src/tools/teams.rs`. [owner:api-engineer]
- [ ] [2.3] Create `src/tools/messages.ts` — `teamsMessages(teamName: string, channelName?: string, count?: number): Promise<string>`. Validate teamName non-empty. Build args: `-Action messages -TeamName '{teamName}'`, append `-ChannelName '{channelName}'` if present, append `-Count {count}` if present. Port from `teams_messages` in `crates/nv-daemon/src/tools/teams.rs`. [owner:api-engineer]
- [ ] [2.4] Create `src/tools/channels.ts` — `teamsChannels(teamName: string): Promise<string>`. Validate teamName non-empty. Call `sshCloudPc("graph-teams.ps1", "-Action channels -TeamName '{teamName}'")`. Port from `teams_channels` in `crates/nv-daemon/src/tools/teams.rs`. [owner:api-engineer]
- [ ] [2.5] Create `src/tools/presence.ts` — `teamsPresence(user: string): Promise<string>`. Validate user non-empty. Call `sshCloudPc("graph-teams.ps1", "-Action presence -User '{user}'")`. Port from `teams_presence` in `crates/nv-daemon/src/tools/teams.rs`. [owner:api-engineer]
- [ ] [2.6] Create `src/tools/send.ts` — `teamsSend(chatId: string, message: string): Promise<string>`. Validate chatId and message non-empty. Call `sshCloudPc("graph-teams.ps1", "-Action send -ChatId '{chatId}' -Message '{message}'")`. [owner:api-engineer]
- [ ] [2.7] Create `src/tools/index.ts` — barrel export all 6 handlers. [owner:api-engineer]

## Phase 3: Hono HTTP Server

- [ ] [3.1] Create `src/index.ts` — Hono app with pino logger middleware. Read `PORT` from env (default 4005). Register `GET /health` returning `{status: "ok", service: "teams-svc"}`. [owner:api-engineer]
- [ ] [3.2] Add HTTP routes to `src/index.ts`: `GET /chats` (query: limit), `GET /chats/:id` (query: limit), `GET /channels` (query: team_name required), `POST /search` (body: team_name required, channel_name?, count?), `GET /presence` (query: user required), `POST /send` (body: chat_id, message). Each route calls the corresponding tool handler. Success: `{ok: true, data}`. Error: `{ok: false, error}` with status 400 (validation), 502 (script error), 503 (CloudPC unreachable), 500 (internal). [owner:api-engineer]

## Phase 4: MCP Server

- [ ] [4.1] Create `src/mcp.ts` — MCP server with stdio transport. Register 6 tools with JSON Schema input definitions matching the Rust tool definitions in `crates/nv-daemon/src/tools/teams.rs`: `teams_list_chats` (optional: limit), `teams_read_chat` (required: chat_id; optional: limit), `teams_messages` (required: team_name; optional: channel_name, count), `teams_channels` (required: team_name), `teams_presence` (required: user), `teams_send` (required: chat_id, message). Each tool handler calls the same function as the HTTP route. [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Scaffold | `pnpm --filter @nova/teams-svc typecheck` passes. `pnpm --filter @nova/teams-svc build` produces `dist/index.js`. |
| 2 Handlers | Typecheck passes with all 6 handler files. |
| 3 HTTP | Service starts on port 4005, `curl http://127.0.0.1:4005/health` returns `{status: "ok", service: "teams-svc"}`. |
| 4 MCP | MCP server binary starts with `--mcp` flag (or stdio mode) and lists 6 tools. |
| **Final** | `pnpm --filter @nova/teams-svc build` succeeds. All routes respond correctly when CloudPC is reachable. Health check passes. |
