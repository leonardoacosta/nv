# Proposal: MS Graph Teams Tools

## Summary

Add 4 Microsoft Graph API tools to the Nova daemon for Teams interaction:
`teams_channels`, `teams_messages`, `teams_send`, and `teams_presence`.

## Motivation

Nova can interact with Jira, GitHub, Slack channels, and Home Assistant but has
no read/write access to Microsoft Teams. Teams is the primary communication
platform for many client organizations. These tools close that gap.

## Design

### Auth: Client Credentials OAuth2

Token endpoint: `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`

Form-encoded POST with:
- `grant_type=client_credentials`
- `client_id`, `client_secret`
- `scope=https://graph.microsoft.com/.default`

Token cached in `Arc<Mutex<Option<(String, Instant)>>>` — refreshed automatically
on expiry (30-second buffer). Reuses the existing `MsGraphAuth` + `TeamsClient`
from `crates/nv-daemon/src/channels/teams/`.

### Env Vars

All present in `~/.nv/env`:
- `MS_GRAPH_CLIENT_ID`
- `MS_GRAPH_CLIENT_SECRET`
- `MS_GRAPH_TENANT_ID`

Optional default team: `NV_TEAMS_TEAM_ID` (env var). Falls back to `[teams].team_id`
in `nv.toml` if available. `team_id` is optional on all tool inputs when this is set.

### Tools

| Tool | Method | Auth | Confirmation |
|------|--------|------|-------------|
| `teams_channels` | GET /teams/{id}/channels | client_creds | None — read-only |
| `teams_messages` | GET /teams/{id}/channels/{id}/messages | client_creds | None — read-only |
| `teams_send` | POST /teams/{id}/channels/{id}/messages | client_creds | PendingAction required |
| `teams_presence` | GET /users/{user}/presence | client_creds | None — read-only |

### Implementation Files

- **New:** `crates/nv-daemon/src/tools/teams.rs` — tool handlers, client construction,
  tool definitions, HTML stripper, truncation helper
- **Modified:** `crates/nv-daemon/src/tools/mod.rs` — dispatch arms in
  `execute_tool_send`, teams definitions in `register_tools()`, tool count updated
- **Modified:** `crates/nv-daemon/src/orchestrator.rs` — `humanize_tool` entries for
  Teams tools

### Error Handling

| Status | Message |
|--------|---------|
| 401 | Auth invalid — token expired, check credentials |
| 403 | Insufficient permissions — lists required Azure AD permission |
| 404 | Team or user not found |
| 429 | Rate limited — retry via `post_with_retry` (up to 3 times) |

## Azure AD Permissions Required

- `Channel.ReadBasic.All` — list channels
- `ChannelMessage.Read.All` — read messages
- `ChannelMessage.Send` — send messages
- `Presence.Read.All` — check user presence

## Alternatives Considered

- **Webhook relay only** — already exists for inbound; this adds outbound tool access
- **User delegated auth** — requires interactive sign-in flow, not suitable for daemon
