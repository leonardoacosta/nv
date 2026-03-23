# Implementation Tasks

<!-- beads:epic:TBD -->

## Config Layer

- [x] [1.1] [P-1] Add `ms_graph_tenant_id: Option<String>` to `Secrets` struct in `crates/nv-core/src/config.rs` — sourced from `MS_GRAPH_TENANT_ID` env var, add to `Secrets::from_env()` [owner:api-engineer]
- [x] [1.2] [P-1] Add `team_id: Option<String>` field to `TeamsConfig` in `crates/nv-core/src/config.rs` — default team for tool operations (distinct from `team_ids` which is the watch list) [owner:api-engineer]

## TeamsClient Extensions

- [x] [2.1] [P-1] Add `PresenceResponse` struct to `crates/nv-daemon/src/channels/teams/types.rs` — fields: `availability` (String), `activity` (String) [owner:api-engineer]
- [x] [2.2] [P-1] Add `ChannelMessage` response struct to `crates/nv-daemon/src/channels/teams/types.rs` — fields: `id`, `created_date_time`, `body` (with `content` and `content_type`), `from` (with `user` containing `display_name`) [owner:api-engineer]
- [x] [2.3] [P-1] Add `get_channel_messages(team_id, channel_id, top: u32)` method to `TeamsClient` in `crates/nv-daemon/src/channels/teams/client.rs` — `GET /teams/{team_id}/channels/{channel_id}/messages?$top={top}`, returns `Vec<ChannelMessage>` [owner:api-engineer]
- [x] [2.4] [P-1] Add `get_user_presence(user: &str)` method to `TeamsClient` in `crates/nv-daemon/src/channels/teams/client.rs` — `GET /users/{user}/presence`, returns `PresenceResponse` [owner:api-engineer]

## Tool Module

- [x] [3.1] [P-1] Create `crates/nv-daemon/src/tools/teams.rs` — module doc comment, `build_teams_client()` helper that constructs `MsGraphAuth` + `TeamsClient` from tenant_id (env > config), client_id, client_secret; returns error if credentials missing [owner:api-engineer]
- [x] [3.2] [P-1] Implement `teams_channels(client, team_id)` handler in `tools/teams.rs` — call `client.list_channels(team_id)`, format as readable list (ID, name, description) [owner:api-engineer]
- [x] [3.3] [P-1] Implement `teams_messages(client, team_id, channel_id)` handler in `tools/teams.rs` — call `client.get_channel_messages(team_id, channel_id, 20)`, format as message list (sender, time, content preview truncated to 200 chars) [owner:api-engineer]
- [x] [3.4] [P-1] Implement `teams_presence(client, user)` handler in `tools/teams.rs` — call `client.get_user_presence(user)`, format as `"Sarah (sarah@civalent.com): Available — InACall"` [owner:api-engineer]
- [x] [3.5] [P-1] Add `teams_tool_definitions()` in `tools/teams.rs` — 4 ToolDefinition structs: `teams_channels` (optional team_id), `teams_messages` (optional team_id, required channel_id), `teams_send` (optional team_id, required channel_id, required message), `teams_presence` (required user) [owner:api-engineer]

## Tool Registration & Dispatch

- [x] [4.1] [P-1] Add `pub mod teams;` to `crates/nv-daemon/src/tools/mod.rs` [owner:api-engineer]
- [x] [4.2] [P-1] Register Teams tools in `register_tools()` — call `teams::teams_tool_definitions()` and extend tool list [owner:api-engineer]
- [x] [4.3] [P-1] Add dispatch arms in `execute_tool()` for `teams_channels`, `teams_messages`, `teams_presence` — resolve team_id (input > config default), build client, call handler, return `ToolResult::Immediate` [owner:api-engineer]
- [x] [4.4] [P-1] Add dispatch arm for `teams_send` — validate channel_id and message params, build description preview, return `ToolResult::PendingAction` with `ActionType::ChannelSend` and payload containing team_id, channel_id, message [owner:api-engineer]

## Verify

- [x] [5.1] `cargo build` passes [owner:api-engineer]
- [x] [5.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [5.3] `cargo test` — existing tests pass [owner:api-engineer]
- [x] [5.4] Unit tests: `build_teams_client()` returns error when credentials missing, succeeds with valid env [owner:api-engineer]
- [x] [5.5] Unit tests: `teams_tool_definitions()` returns 4 tools with correct names and schemas [owner:api-engineer]
- [x] [5.6] Unit tests: message formatting (truncation at 200 chars, empty list handling) [owner:api-engineer]
- [x] [5.7] Unit tests: presence formatting (availability + activity rendering) [owner:api-engineer]
- [ ] [5.8] [user] Manual test: send "List Teams channels" via Telegram, verify formatted response [owner:api-engineer]
