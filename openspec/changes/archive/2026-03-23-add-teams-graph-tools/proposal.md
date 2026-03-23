# Proposal: Add Microsoft Graph Teams Tools

## Change ID
`add-teams-graph-tools`

## Summary

Four read-only + one write Teams tool powered by the MS Graph REST API. Reuses the existing
`MsGraphAuth` OAuth2 client from `channels/teams/oauth.rs` and the `TeamsClient` from
`channels/teams/client.rs`. The tools module provides Claude-callable tool definitions and
dispatch — separate from the channel adapter which handles inbound webhook relay.

Tools: `teams_channels` (list channels), `teams_messages` (read channel messages),
`teams_send` (send message with PendingAction confirmation), `teams_presence` (user presence).

## Context
- Extends: `crates/nv-daemon/src/tools/mod.rs` (tool registration + dispatch)
- Reuses: `crates/nv-daemon/src/channels/teams/oauth.rs` (`MsGraphAuth`), `crates/nv-daemon/src/channels/teams/client.rs` (`TeamsClient`)
- Related: `crates/nv-daemon/src/tools/calendar.rs` (same pattern — API client module + tool definitions), `crates/nv-core/src/config.rs` (TeamsConfig already exists)
- Auth env vars: `MS_GRAPH_CLIENT_ID` (already in Secrets), `MS_GRAPH_CLIENT_SECRET` (already in Secrets), `MS_GRAPH_TENANT_ID` (needs adding to Secrets)
- Config: `[teams]` section with `tenant_id`, `team_ids`, `channel_ids` already defined in `TeamsConfig`

## Motivation

Nova's existing Teams integration is a webhook relay — it receives inbound messages via MS Graph
subscriptions and sends replies through the Channel trait. But there are no Claude-callable tools
for Teams. Leo works with Civalent/Brown & Brown via Microsoft Teams and needs Nova to:

1. **List channels** — "What channels does the Civalent team have?"
2. **Read messages** — "What's been discussed in the General channel today?"
3. **Send messages** — "Post a status update to the Engineering channel" (with confirmation)
4. **Check presence** — "Is Sarah available on Teams right now?"

These are tool-level operations that the agent loop dispatches, distinct from the channel-level
inbound relay. The existing `MsGraphAuth` and `TeamsClient` already handle OAuth2 and API calls —
the tools module wraps them with tool definitions and dispatch.

## Requirements

### Req-1: Add MS_GRAPH_TENANT_ID to Secrets

Add `ms_graph_tenant_id: Option<String>` to the `Secrets` struct in `config.rs`, sourced from
`MS_GRAPH_TENANT_ID` env var. The `TeamsConfig.tenant_id` field already exists in config TOML, but
the tools need a way to resolve tenant ID from env (consistent with the client_id/secret pattern).

Resolution order: `MS_GRAPH_TENANT_ID` env var > `[teams].tenant_id` in config > error.

### Req-2: teams.rs Tool Module

New file `crates/nv-daemon/src/tools/teams.rs` with:

- Module-level doc comment describing the 4 tools and auth
- `build_teams_client()` helper that constructs `MsGraphAuth` + `TeamsClient` from secrets/config
- Response types for deserialization (presence, message list)
- Four public async tool handler functions
- Error mapping: 401 = auth invalid, 403 = insufficient permissions, 404 = team/channel not found

### Req-3: `teams_channels` Tool

List channels accessible to the bot in a team.

- Reuses: `TeamsClient::list_channels(team_id)`
- Input: `team_id` (optional — defaults to config `team_ids[0]` or the `[teams].team_id` config)
- Output: formatted list with channel ID, display name, description
- Read-only, no confirmation needed

### Req-4: `teams_messages` Tool

Get recent messages from a Teams channel.

- Endpoint: `GET /teams/{team-id}/channels/{channel-id}/messages` (add to `TeamsClient`)
- Input: `team_id` (optional, defaults to config), `channel_id` (required)
- Output: last 20 messages with sender name, timestamp, content preview (truncated to 200 chars)
- Read-only, no confirmation needed

### Req-5: `teams_send` Tool (with PendingAction)

Send a message to a Teams channel.

- Reuses: `TeamsClient::send_channel_message(team_id, channel_id, content)`
- Input: `team_id` (optional, defaults to config), `channel_id` (required), `message` (required)
- **PendingAction**: Returns `ToolResult::PendingAction` with `ActionType::ChannelSend` and a
  description like `Send to Teams #General: "Status update..."`. User must confirm via Telegram
  inline keyboard before execution.
- The execution handler reuses the existing `execute_channel_send` flow or adds a parallel
  `execute_teams_send` if the payload shape differs.

### Req-6: `teams_presence` Tool

Check a user's presence/availability status on Teams.

- Endpoint: `GET /users/{user-id-or-upn}/presence` (add to `TeamsClient`)
- Input: `user` (required — email/UPN like `sarah@civalent.com` or user ID)
- Output: availability (Available, Busy, DoNotDisturb, Away, Offline), activity (InACall,
  InAMeeting, Presenting, etc.)
- Read-only, no confirmation needed
- Requires `Presence.Read.All` application permission in Azure AD

### Req-7: TeamsClient Extensions

Add two new methods to `crates/nv-daemon/src/channels/teams/client.rs`:

- `get_channel_messages(team_id, channel_id, top: u32)` — `GET /teams/{team-id}/channels/{channel-id}/messages?$top={top}`
- `get_user_presence(user_id_or_upn: &str)` — `GET /users/{user}/presence`

Both use the existing `auth.get_token()` + Bearer header pattern. Both read-only.

### Req-8: Tool Registration & Dispatch

- Add `teams_tool_definitions()` returning 4 `ToolDefinition` structs
- Register in `register_tools()` alongside existing tool sets
- Add dispatch arms in `execute_tool()` for `teams_channels`, `teams_messages`, `teams_send`,
  `teams_presence`
- `teams_send` dispatches as `PendingAction`; other three return `ToolResult::Immediate`

### Req-9: Default Team ID Resolution

Add a `team_id` field to `TeamsConfig`:

```toml
[teams]
tenant_id = "xxx"
team_id = "xxx"       # <-- new: default team for tool operations
team_ids = [...]      # existing: teams to watch for inbound
channel_ids = [...]   # existing: channels to filter
```

Tools that accept optional `team_id` fall back to `config.teams.team_id` when not provided.

## Scope
- **IN**: 4 tool definitions + dispatch, `teams.rs` tool module, 2 new `TeamsClient` methods (get_channel_messages, get_user_presence), `ms_graph_tenant_id` in Secrets, `team_id` in TeamsConfig, PendingAction for teams_send
- **OUT**: Adaptive cards / rich message formatting, file attachments, meeting management, Teams bot framework integration, Teams channel adapter modifications (inbound relay unchanged), calendar via MS Graph (separate spec), direct/chat messages (channel messages only)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/teams.rs` | New: tool module with build_teams_client(), response types, 4 handler functions, formatting |
| `crates/nv-daemon/src/tools/mod.rs` | Add `pub mod teams;`, `teams_tool_definitions()` call in `register_tools()`, 4 dispatch arms in `execute_tool()` |
| `crates/nv-daemon/src/channels/teams/client.rs` | Add `get_channel_messages()` and `get_user_presence()` methods |
| `crates/nv-daemon/src/channels/teams/types.rs` | Add `PresenceResponse`, `ChannelMessageResponse` types |
| `crates/nv-core/src/config.rs` | Add `ms_graph_tenant_id` to Secrets, add `team_id: Option<String>` to TeamsConfig |

## Risks
| Risk | Mitigation |
|------|-----------|
| Azure AD app may lack Presence.Read.All permission | teams_presence returns clear error: "Presence permission not granted in Azure AD app registration" |
| Tenant ID resolution ambiguity (env vs config) | Document resolution order: env > config > error. Single source of truth per deployment. |
| Channel message API requires ChannelMessage.Read.All | Verify app permissions in Azure AD. Return clear error on 403. |
| Rate limiting on MS Graph (10,000 req/10min per app) | Unlikely at single-user scale. TeamsClient already handles 429 with Retry-After. |
| MsGraphAuth is currently constructed per-TeamsChannel instance | Tools create their own MsGraphAuth instance. Token caching is per-instance (acceptable — both cache independently, no conflict). |
| teams_send could send to wrong channel | PendingAction confirmation includes channel name in description. User must explicitly approve. |
