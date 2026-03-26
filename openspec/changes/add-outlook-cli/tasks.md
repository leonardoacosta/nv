# Implementation Tasks

<!-- beads:epic:nv-2884 -->

## API Batch

- [x] [1.1] [P-1] Create `crates/nv-tools/src/tools/outlook.rs` — module-level doc, imports, `GRAPH_BASE` const, `GraphUserAuth` struct with `access_token`, `refresh_token`, `expires_at_unix`, `client_id`, `tenant_id`, `http: Client` fields [owner:api-engineer]
- [x] [1.2] [P-1] Implement `GraphUserAuth::from_env_or_cache() -> Result<Self>` — read `MS_GRAPH_CLIENT_ID`/`MS_GRAPH_TENANT_ID` from env, load token from `NV_GRAPH_USER_TOKEN_PATH` (default `~/.config/nv/graph-user-token.json`), run device-code flow if missing/expired-no-refresh, save acquired token [owner:api-engineer]
- [x] [1.3] [P-1] Implement `GraphUserAuth::get_token(&mut self) -> Result<String>` — return cached token if valid (60s buffer), otherwise exchange refresh token, update cache file, return new token; error with actionable message if refresh fails [owner:api-engineer]
- [x] [1.4] [P-1] Add `save(&self, path: &Path) -> Result<()>` — serialize to JSON, write with `0o600` permissions; add `TokenCache` and `DeviceCodeResponse`/`TokenResponse` serde types [owner:api-engineer]
- [x] [1.5] [P-1] Implement `fn get_json(auth: &mut GraphUserAuth, url: &str, http: &Client) -> Result<Value>` helper — GET with `Authorization: Bearer {token}`, handle 401 (force refresh + retry once), 429 (sleep + retry), other errors as `anyhow::bail!` [owner:api-engineer]
- [x] [1.6] [P-1] Add `outlook_inbox(auth: &mut GraphUserAuth, folder: Option<&str>, count: u32, unread_only: bool) -> Result<String>` — resolve folder ID if custom, call Graph messages endpoint, format numbered blocks [owner:api-engineer]
- [x] [1.7] [P-1] Add response types: `MailMessage`, `EmailAddressWrapper`, `EmailAddress`, `MailListResponse` with `#[serde(rename_all = "camelCase")]` [owner:api-engineer]
- [x] [1.8] [P-1] Add `outlook_calendar(auth: &mut GraphUserAuth, days_ahead: u32, max_events: u32) -> Result<String>` — calendarView query with RFC3339 bounds, group by day header if `days_ahead > 1`, format timed/all-day/cancelled events [owner:api-engineer]
- [x] [1.9] [P-1] Add calendar response types: `CalendarEvent`, `DateTimeTimeZone`, `CalendarOrganizer`, `CalendarAttendee`, `CalendarLocation` with camelCase serde [owner:api-engineer]
- [x] [1.10] [P-2] Add `outlook_read_email(auth: &mut GraphUserAuth, message_id: &str) -> Result<String>` — GET single message, strip HTML body using regex helper, format headers + truncated body (max 4000 chars) [owner:api-engineer]
- [x] [1.11] [P-2] Add `strip_html(html: &str) -> String` helper (inline or imported) — strip `<[^>]+>` tags, collapse whitespace; note in comment that teams.rs has the same pattern [owner:api-engineer]
- [x] [1.12] [P-2] Add `outlook_tool_definitions() -> Vec<ToolDefinition>` — three definitions: `outlook_inbox` (optional: folder, count, unread_only), `outlook_calendar` (optional: days_ahead, max_events), `outlook_read_email` (required: message_id) [owner:api-engineer]
- [x] [1.13] [P-3] Unit tests: `outlook_tool_definitions()` count == 3 and names present; `format_inbox_message` with fixture JSON; `format_calendar_event` for timed, all-day, and cancelled; `strip_html` basic cases [owner:api-engineer]

## DB Batch

(none — no schema changes)

## UI Batch

(none — CLI/MCP tool only)

## E2E Batch

- [x] [2.1] [P-1] Register `pub mod outlook;` in `crates/nv-tools/src/tools/mod.rs` [owner:api-engineer]
- [x] [2.2] [P-1] Add `tools.extend(outlook::outlook_tool_definitions());` to `stateless_tool_definitions()` in `crates/nv-tools/src/dispatch.rs` [owner:api-engineer]
- [x] [2.3] [P-1] Add dispatch arms for `outlook_inbox`, `outlook_calendar`, `outlook_read_email` in `dispatch_stateless()` — extract params from `args`, call `GraphUserAuth::from_env_or_cache()`, call handler [owner:api-engineer]
- [x] [2.4] [P-2] Update tool-count comment in `dispatch.rs` (e.g. `// + 3 outlook`) [owner:api-engineer]
- [x] [2.5] [P-2] Integration test (behind `#[cfg(feature = "integration")]`): `stateless_tool_definitions()` contains `outlook_inbox`, `outlook_calendar`, `outlook_read_email` by name [owner:api-engineer]
- [x] [2.6] [P-2] `cargo build -p nv-tools` passes; `cargo clippy -p nv-tools -- -D warnings` passes [owner:api-engineer]
- [x] [2.7] [P-3] `cargo test -p nv-tools` — all unit tests pass [owner:api-engineer]
- [ ] [2.8] [user] Manual: set `MS_GRAPH_CLIENT_ID` and `MS_GRAPH_TENANT_ID`; invoke `outlook_inbox` tool; confirm device-code flow prints user_code + verification_uri to stderr and token is written to `~/.config/nv/graph-user-token.json`
- [ ] [2.9] [user] Manual: invoke `outlook_inbox` a second time; confirm it uses cached token (no device-code prompt)
- [ ] [2.10] [user] Manual: invoke `outlook_calendar` and `outlook_read_email` with a message id from inbox output; verify readable output
