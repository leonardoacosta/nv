# Proposal: Add Teams Service

## Change ID
`add-teams-svc`

## Summary

Build the Teams tool service (`teams-svc`) as a Hono HTTP server on port 4005 with MCP stdio
transport. All six Teams operations execute via SSH to the CloudPC host, where PowerShell scripts
handle Graph API authentication. The service is a thin wrapper: HTTP/MCP request -> SSH exec
PowerShell -> parse output -> return JSON.

## Context
- Extends: `packages/tools/teams-svc/` (new service, follows scaffold-tool-service template)
- Depends on: `scaffold-tool-service` (Wave 1) — provides the shared Hono+MCP service scaffold
- Ports from: `crates/nv-daemon/src/tools/teams.rs` (SSH-to-CloudPC pattern),
  `crates/nv-daemon/src/tools/cloudpc.rs` (SSH exec helper),
  `packages/tools/teams-cli/` (TypeScript types and formatting helpers)
- Related: `add-graph-svc` (same wave, same SSH-to-CloudPC pattern for calendar/ADO),
  `add-fleet-deploy` (systemd service file `nova-teams-svc.service` at PORT=4005)
- Architecture: `docs/plan/nova-v10/scope-lock.md` defines teams-svc at :4005
- SSH host: `cloudpc` (SSH config name), script path: `C:\Users\leo.346-CPC-QJXVZ\graph-teams.ps1`

## Motivation

The Rust daemon's Teams tools route through `ssh cloudpc "powershell ... graph-teams.ps1 -Action
..."` and this pattern works reliably. The v10 tool fleet architecture requires each tool domain
to run as an independent Hono service. `teams-svc` ports the exact same SSH-to-CloudPC pattern
into TypeScript, adding HTTP routes for the dashboard and MCP tool definitions for Agent SDK
native discovery.

The existing `teams-cli` package uses direct Graph API calls with client_credentials, which is an
alternative auth path. `teams-svc` deliberately uses SSH-to-CloudPC instead because the PowerShell
scripts on the CloudPC manage their own delegated OAuth tokens (device-code flow) which have access
to richer data (chats, channel messages) than app-only permissions allow.

## Requirements

### Req-1: SSH-to-CloudPC Execution Helper

`src/ssh.ts` — a `sshCloudPc(script, args)` function:

- Spawns `ssh -o ConnectTimeout=10 cloudpc "powershell -ExecutionPolicy Bypass -Command \"& { . C:\Users\leo.346-CPC-QJXVZ\{script} {args} }\"""`
- Returns stdout as a string, filtering out `WARNING:`, `vulnerable`, `upgraded`, `security fix`
  noise lines (matching the Rust `cloudpc.rs` filter)
- On SSH connection failure (refused, timeout, no route): throws with
  `"CloudPC unreachable -- cannot connect via SSH"`
- On non-zero exit: throws with stderr content
- Timeout: 30 seconds (SSH `ConnectTimeout=10` + script execution time)

### Req-2: Tool Handlers (6 tools)

`src/tools/` — one handler per tool, each calling `sshCloudPc("graph-teams.ps1", ...)`:

1. `teams_list_chats(limit?: number)` — runs `-Action list`, returns parsed chat list
2. `teams_read_chat(chat_id: string, limit?: number)` — runs `-Action messages -ChatId '{chat_id}' -Count {limit}`
3. `teams_messages(team_name: string, channel_name?: string, count?: number)` — runs `-Action messages -TeamName '{team_name}' [-ChannelName '{channel_name}'] [-Count {count}]`
4. `teams_channels(team_name: string)` — runs `-Action channels -TeamName '{team_name}'`
5. `teams_presence(user: string)` — runs `-Action presence -User '{user}'`
6. `teams_send(chat_id: string, message: string)` — runs `-Action send -ChatId '{chat_id}' -Message '{message}'`

Each handler returns the CloudPC script output as a string. The service does not parse or reformat
the PowerShell output beyond the noise-line filter -- the scripts produce Claude-readable text.

Input validation: reject empty required fields with 400 status. Limit values clamped to 1-50.

### Req-3: Hono HTTP Server

`src/index.ts` — Hono app on port from `PORT` env var (default 4005):

| Method | Route | Handler | Query/Body Params |
|--------|-------|---------|-------------------|
| GET | `/chats` | teams_list_chats | `?limit=20` |
| GET | `/chats/:id` | teams_read_chat | `?limit=20` |
| GET | `/channels` | teams_channels | `?team_name=` (required) |
| POST | `/search` | teams_messages | `{team_name, channel_name?, count?}` |
| GET | `/presence` | teams_presence | `?user=` (required) |
| POST | `/send` | teams_send | `{chat_id, message}` |
| GET | `/health` | health check | returns `{status: "ok", service: "teams-svc"}` |

Response format: `{ok: true, data: <string>}` on success, `{ok: false, error: <string>}` on
failure. Content-Type: application/json.

Middleware: pino logger for request logging.

### Req-4: MCP Tool Definitions

`src/mcp.ts` — MCP server with stdio transport exposing 6 tools:

| Tool Name | Description | Required Params | Optional Params |
|-----------|-------------|-----------------|-----------------|
| teams_list_chats | List recent Teams chats and DMs | none | limit (number, 1-50, default 20) |
| teams_read_chat | Read messages from a specific chat | chat_id (string) | limit (number, 1-50, default 20) |
| teams_messages | Read messages from a Teams channel | team_name (string) | channel_name (string), count (number) |
| teams_channels | List channels in a Teams team | team_name (string) | none |
| teams_presence | Get user presence/availability | user (string) | none |
| teams_send | Send a message to a Teams chat | chat_id (string), message (string) | none |

Each MCP tool calls the same handler as the HTTP route.

### Req-5: Error Handling

- SSH connection failure: HTTP 503, error message includes "CloudPC unreachable"
- SSH script error (non-zero exit): HTTP 502, error message includes script stderr
- Missing required params: HTTP 400
- Internal error: HTTP 500
- All errors logged via pino with full context

### Req-6: Package Configuration

`packages/tools/teams-svc/package.json`:
- Name: `@nova/teams-svc`
- Scripts: `build` (esbuild bundle), `dev` (tsx watch), `start` (node dist/index.js)
- Dependencies: `hono`, `pino` (from scaffold template)
- Dev dependencies: `@types/node`, `esbuild`, `tsx`, `typescript`

`packages/tools/teams-svc/tsconfig.json`: strict mode, Node 20 target, ESM.

## Scope
- **IN**: `packages/tools/teams-svc/` package, SSH exec helper, 6 tool handlers, Hono HTTP routes,
  MCP tool definitions, health check endpoint, pino logging, error handling
- **OUT**: Direct Graph API calls (uses SSH-to-CloudPC exclusively), `teams-cli` changes,
  `nv-daemon` changes, Traefik routing config, systemd service file (handled by `add-fleet-deploy`),
  MCP registration in `~/.claude/mcp.json` (handled by `register-mcp-servers`)

## Impact
| Area | Change |
|------|--------|
| `packages/tools/teams-svc/` | New package -- src/index.ts (Hono server), src/ssh.ts (CloudPC helper), src/tools/*.ts (6 handlers), src/mcp.ts (MCP server), package.json, tsconfig.json |
| `packages/tools/teams-cli/` | No changes -- existing CLI remains as-is |
| Doppler | No new secrets -- SSH uses key-based auth via ssh-agent |

## Risks
| Risk | Mitigation |
|------|-----------|
| CloudPC offline makes all 6 tools unavailable | Health check reports CloudPC connectivity; 503 with clear message; dashboard can show service-degraded status |
| PowerShell script output format changes | Service passes through raw output; no brittle parsing to break |
| SSH key not forwarded in systemd context | systemd service runs as user with `~/.ssh/` config; `ssh-agent` must be running (document in deploy notes) |
| Concurrent SSH connections under load | Node child_process is non-blocking; CloudPC handles multiple SSH sessions; no pooling needed for expected 1-5 concurrent agents |

## Dependencies
- `scaffold-tool-service` -- provides the shared Hono+MCP template this service follows
