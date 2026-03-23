# Tasks: MS Graph Teams Tools

## Status: Complete

All tasks implemented and verified with `cargo build` (clean) and `cargo clippy` (no warnings).

---

## Task List

- [x] **T1 — teams.rs tool handlers**
  Create `crates/nv-daemon/src/tools/teams.rs` with:
  - `build_teams_client()` — constructs `TeamsClient` from `Secrets` + optional `TeamsConfig`
  - `teams_channels()` — list channels, friendly error mapping for 401/403/404
  - `teams_messages()` — get recent messages, HTML stripping, 200-char truncation
  - `teams_presence()` — check user availability/activity
  - `teams_tool_definitions()` — 4 `ToolDefinition` structs for Anthropic API
  - Internal: `strip_html()`, `truncate_to_chars()`
  - Tests: build_teams_client validation, tool definitions shape, helper unit tests

- [x] **T2 — mod.rs dispatch arms**
  Add to `execute_tool_send` in `crates/nv-daemon/src/tools/mod.rs`:
  - `teams_channels` arm — resolves team_id from input or `NV_TEAMS_TEAM_ID`
  - `teams_messages` arm — requires channel_id, team_id optional
  - `teams_send` arm — `ToolResult::PendingAction` with `ActionType::ChannelSend`
  - `teams_presence` arm — requires user (email/UPN or object ID)
  Also: add second copy in `execute_tool` (agent path) and `register_tools()` count/assertions

- [x] **T3 — orchestrator humanize_tool**
  Add Teams entries to `humanize_tool()` in `crates/nv-daemon/src/orchestrator.rs`:
  - `teams_channels | teams_messages | teams_presence` → "Checking Teams..."
  - `teams_send` → "Sending to Teams..."

- [x] **T4 — Cargo build + clippy gate**
  - `cargo build` passes (0 errors)
  - `cargo clippy` passes (0 warnings)

## Notes

- `teams_send` reuses `ActionType::ChannelSend` — no new variant needed in `nv_core::types`
- Token caching is handled by the existing `MsGraphAuth` in `channels/teams/oauth.rs`
- The `mod.rs` declaration for `pub mod teams` was already present before this change
- Tool count in `register_tools_returns_expected_count` test updated to 91 (includes 4 Teams tools)
