# Proposal: Add Teams CLI Tool

## Change ID
`add-teams-cli`

## Summary

Create a standalone TypeScript CLI (`teams-cli`) that wraps the MS Graph API for Teams operations, invokable by Nova via Bash as a direct alternative to the existing CloudPC/SSH/PowerShell path. Installed to `~/.local/bin/teams-cli`.

## Context
- Extends: `packages/tools/teams-cli/` (new package)
- Related: `crates/nv-daemon/src/tools/teams.rs` (existing Teams tools via CloudPC SSH), archived `add-teams-chat-tools` (TeamsClient Rust patterns), archived `ms-graph-cli-tools` (MsGraphClient dual-auth patterns)
- Auth model: client_credentials (same as existing `teams_presence` + `teams_send` in nv-daemon) using `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID` from Doppler
- Note: `chats` and `read-chat` require `Chat.Read.All` application permission (delegated `/me/chats` is not available to client_credentials); `channels` and `messages` use application-level `ChannelMessage.Read.All`; `send` uses `ChatMessage.Send`

## Motivation

The current Teams tools in `nv-daemon` route through CloudPC SSH → PowerShell → `graph-teams.ps1`, which creates a hard dependency on the Windows VM being reachable. A standalone CLI using direct Graph API calls via client_credentials gives Nova a homelab-native, no-SSH path for Teams operations and can be invoked via any Bash tool call without daemon involvement.

## Requirements

### Req-1: CLI Entry Point and Command Structure

`src/index.ts` using commander with six subcommands:

- `chats [--limit N]` — list recent chats (DMs + group chats)
- `read-chat <id> [--limit N]` — read messages from a chat
- `channels <team-id>` — list channels in a team
- `messages <team-id> <channel-id> [--limit N]` — read channel messages
- `presence <user>` — check user presence/availability
- `send <chat-id> <message>` — send a message to a chat

Default limit: 20. Max limit: 50.

### Req-2: Auth — Client Credentials

`src/auth.ts` — `MsGraphClient` class:

- Reads `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID` from environment
- Fetches token via `POST https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token` with `grant_type=client_credentials`, `scope=https://graph.microsoft.com/.default`
- Caches token in memory for its lifetime (typically 1 hour)
- On missing env vars: prints a clear "not configured" message to stderr and exits with code 1
- Exposes `get(url): Promise<unknown>` and `post(url, body): Promise<unknown>` helpers that inject `Authorization: Bearer {token}`

### Req-3: Commands Implementation

`src/commands/` — one file per command group:

- `chats.ts` — `GET /chats?$top={limit}&$expand=members&$orderby=lastMessageReceivedDateTime desc` — formatted output: topic (or "DM: {other member}" for oneOnOne), type badge, last activity
- `channels.ts` — `GET /teams/{team-id}/channels` — formatted list with channel id and description
- `messages.ts` — `GET /teams/{team-id}/channels/{channel-id}/messages?$top={limit}` — formatted: sender, timestamp, plain text body (HTML stripped)
- `presence.ts` — `GET /users/{user}/presence` — single-line: `{user}: {availability} — {activity}`
- `send.ts` — `POST /chats/{chat-id}/messages` with `{"body":{"content":"{message}"}}` — confirms "Sent." on success

### Req-4: Output Format

All output to stdout as formatted plain text optimized for Claude readability. No JSON output mode. HTML in message bodies stripped to plain text (regex strip `<[^>]+>`). Timestamps formatted as relative time where helpful (e.g., "3h ago", "Mar 15").

Example `chats --limit 5` output:
```
Recent Chats (5)
DM: Sarah Martinez — last active 2h ago
Group: Wholesale Architecture — last active 5h ago
DM: Alex Johnson — last active 1d ago
Meeting: Sprint Review — last active Mar 20
Group: Platform Engineering — last active Mar 18
```

Example `read-chat <id> --limit 3` output:
```
Chat: Wholesale Architecture (last 3 messages)
[2h ago] Sarah Martinez: Sounds good, let's sync at 2pm
[3h ago] Alex Johnson: The migration is blocked on the schema review
[4h ago] Leo Acosta: Can someone take a look at PR #4521?
```

### Req-5: Build and Install

`package.json` with:
- `build` script: `esbuild src/index.ts --bundle --platform=node --outfile=dist/teams-cli.js`
- `install-cli` script: `cp dist/teams-cli.js ~/.local/bin/teams-cli && chmod +x ~/.local/bin/teams-cli`
- Shebang `#!/usr/bin/env node` prepended to dist output via build step

`tsconfig.json` targeting Node 20 with strict mode.

Dependencies: `commander`, `esbuild` (dev). No additional runtime deps beyond Node built-ins and commander — HTTP calls via built-in `fetch` (Node 18+).

### Req-6: Error Handling

- Missing required arguments: print usage hint and exit 1
- Graph API 403 (insufficient permissions): print descriptive message naming the required permission, exit 1
- Graph API 404 (resource not found): print "Not found: {id}", exit 1
- Network error: print error message, exit 1
- All errors to stderr; all output to stdout

## Scope
- **IN**: `packages/tools/teams-cli/` package, six commands, client_credentials auth, plain text output, esbuild bundle, install to `~/.local/bin`
- **OUT**: `nv-daemon` changes, delegated/device-code auth, JSON output flag, pagination beyond limit cap, attachment handling, creating new chats or teams

## Impact
| Area | Change |
|------|--------|
| `packages/tools/teams-cli/` | New package — CLI entry, auth, 5 command modules, build config |
| `~/.local/bin/teams-cli` | Installed binary (not tracked in git) |
| Doppler | No new secrets — reuses existing `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID` |

## Risks
| Risk | Mitigation |
|------|-----------|
| `Chat.Read.All` may not be granted on the Azure AD app | Test `chats` command first; on 403, document the required permission grant in the error message |
| `ChannelMessage.Read.All` requires admin consent | Same mitigation — exit 1 with permission name on 403 |
| Node 18+ required for built-in `fetch` | Document minimum Node version; shebang uses `env node` |
| `send` has no confirmation prompt | CLI is invoked by Nova which provides its own confirmation layer; no shell prompt needed |
