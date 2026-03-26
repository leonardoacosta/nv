# Implementation Tasks

<!-- beads:epic:nv-kiti -->

## Phase 1 — MsGraphUserAuth (Device-Code Token Manager)

- [x] [1.1] [P-1] Add `MsGraphUserAuth` struct to `crates/nv-daemon/src/channels/teams/oauth.rs` — fields: `access_token: String`, `refresh_token: Option<String>`, `expires_at: Instant`, `client_id: String`, `tenant_id: String`, `http: Client` [owner:api-engineer]
- [x] [1.2] [P-1] Implement `MsGraphUserAuth::from_cache(path: &Path) -> Option<Self>` — deserialize `~/.config/nv/graph-token.json` (or `NV_GRAPH_TOKEN_PATH`), return `None` if missing or parse error [owner:api-engineer]
- [x] [1.3] [P-1] Implement `MsGraphUserAuth::device_code_flow(client_id, tenant_id, scopes) -> Result<Self>` — POST `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/devicecode`, print user_code + verification_uri to stderr, poll token endpoint every `interval` secs until success or 300s timeout [owner:api-engineer]
- [x] [1.4] [P-1] Implement `MsGraphUserAuth::get_token(&self) -> Result<String>` — return cached token if valid (5-min buffer), attempt silent refresh via `refresh_token` grant if near expiry, return `Err("Graph token expired — run `nv auth graph` to re-authenticate")` if refresh fails and no device_code_flow available [owner:api-engineer]
- [x] [1.5] [P-1] Implement `MsGraphUserAuth::save(&self, path: &Path) -> Result<()>` — serialize to JSON `{access_token, refresh_token, expires_at_unix, client_id, tenant_id}`, write with file permissions 0o600 (use `std::fs::set_permissions`) [owner:api-engineer]
- [x] [1.6] [P-2] Add `MsGraphUserAuth::try_load_or_prompt(client_id, tenant_id) -> Result<Self>` — convenience: try from_cache first; if missing/expired-no-refresh, call device_code_flow with Outlook scopes (`Mail.Read Calendars.Read offline_access`) and save result [owner:api-engineer]
- [x] [1.7] [P-3] Unit tests: `from_cache` returns None for missing file, `save` + `from_cache` round-trips JSON, `get_token` returns cached when valid, `get_token` errors when expired and no refresh [owner:api-engineer]

## Phase 2 — tools/outlook.rs

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/tools/outlook.rs` — module declaration, imports, `GRAPH_API_BASE` const, helper `fn strip_html(html: &str) -> String` (reuse pattern from `tools/teams.rs`) [owner:api-engineer]
- [x] [2.2] [P-1] Add `OutlookClient` struct — wraps `MsGraphUserAuth`, provides `get_json(url: &str) -> Result<serde_json::Value>` with Bearer auth and 429 retry [owner:api-engineer]
- [x] [2.3] [P-1] Add `read_outlook_inbox(client: &OutlookClient, folder: Option<&str>, count: u32, unread_only: bool) -> Result<String>` — resolves folder ID if not `Inbox`, calls `GET /me/mailFolders/{folder}/messages?$select=...&$orderby=receivedDateTime desc&$top={count}`, formats output as per Req-2 [owner:api-engineer]
- [x] [2.4] [P-1] Add response types: `MailMessage { id, subject, from: EmailAddressWrapper, received_date_time, is_read, has_attachments, importance, body_preview }`, `EmailAddressWrapper { email_address: EmailAddress }`, `EmailAddress { name, address }`, `MailListResponse { value: Vec<MailMessage> }` — all `#[derive(Deserialize)]` with `#[serde(rename_all = "camelCase")]` [owner:api-engineer]
- [x] [2.5] [P-2] Add folder resolution helper `resolve_folder_id(client: &OutlookClient, folder_name: &str) -> Result<String>` — GET `/me/mailFolders?$top=25`, match `displayName` case-insensitively, return folder `id` or error if not found [owner:api-engineer]
- [x] [2.6] [P-2] Add `read_outlook_calendar(client: &OutlookClient, days_ahead: u32, max_events: u32) -> Result<String>` — builds `startDateTime`/`endDateTime` in RFC3339 UTC, calls `GET /me/calendarView?startDateTime=...&endDateTime=...&$top={max}&$select=...&$orderby=start/dateTime`, formats output as per Req-3 [owner:api-engineer]
- [x] [2.7] [P-2] Add calendar response types: `CalendarEvent { subject, start: DateTimeTimeZone, end: DateTimeTimeZone, organizer: CalendarRecipient, attendees: Vec<Attendee>, location: Option<Location>, is_all_day, is_cancelled, body_preview }`, `DateTimeTimeZone { date_time, time_zone }` — all `#[derive(Deserialize)]` [owner:api-engineer]
- [x] [2.8] [P-2] Format calendar output: group by day header if `days_ahead > 1`, show `[HH:MM–HH:MM]` for timed events and `[All Day]` for all-day events, show location if present, show organizer and attendee count [owner:api-engineer]
- [x] [2.9] [P-3] Add `outlook_tool_definitions() -> Vec<ToolDefinition>` — two definitions: `read_outlook_inbox` (properties: `folder?: string`, `count?: integer`, `unread_only?: boolean`) and `read_outlook_calendar` (properties: `days_ahead?: integer`, `max_events?: integer`) [owner:api-engineer]
- [x] [2.10] [P-3] Unit tests: `strip_html` (covered by teams tests already — reference rather than duplicate), format_inbox with fixture JSON, format_calendar_event for timed and all-day events [owner:api-engineer]

## Phase 3 — tools/ado.rs: query_ado_work_items

- [x] [3.1] [P-1] Add `AdoWorkItem` struct to `tools/ado.rs` — fields: `id: u32`, `fields: AdoWorkItemFields` with `system_id, system_title, system_state, system_work_item_type, system_assigned_to: Option<AdoIdentity>, system_changed_date: Option<String>` — use `#[serde(rename = "System.Title")]` etc. [owner:api-engineer]
- [x] [3.2] [P-1] Add `AdoClient::work_items_by_wiql(&self, project: &str, wiql: &str, limit: usize) -> Result<Vec<AdoWorkItem>>` — POST `/{project}/_apis/wit/wiql?api-version=7.1` with `{query: wiql}`, extract `workItems[].id` from response (max `limit`), batch-fetch via `GET /_apis/wit/workitems?ids={csv}&fields=...&api-version=7.1` [owner:api-engineer]
- [x] [3.3] [P-1] Add `query_ado_work_items(client: &AdoClient, project: &str, assigned_to: &str, state_filter: &str, limit: usize) -> Result<String>` — builds WIQL query from params, calls `work_items_by_wiql`, formats output as per Req-4 [owner:api-engineer]
- [x] [3.4] [P-2] Add `ado_work_items` tool definition to `ado_tool_definitions()` — properties: `project?: string`, `assigned_to?: string` (default `@Me`), `state?: string` (default `active`, accepts `active`/`new`/`resolved`/`all`), `limit?: integer` [owner:api-engineer]
- [x] [3.5] [P-2] Add `AdoClient::from_aad_token(org: &str, token: &str) -> Result<Self>` — constructs client with `Authorization: Bearer {token}` header instead of Basic auth (Req-5 optional enhancement) [owner:api-engineer]
- [x] [3.6] [P-3] Unit tests: `format_work_items` with fixture response, WIQL query string generation, missing `ADO_PROJECT` falls back to error message [owner:api-engineer]

## Phase 4 — tools/mod.rs Registration

- [x] [4.1] [P-1] Add `pub mod outlook;` to `crates/nv-daemon/src/tools/mod.rs` [owner:api-engineer]
- [x] [4.2] [P-1] Add `outlook::outlook_tool_definitions()` to the tool definitions list in `all_tool_definitions()` or equivalent aggregator [owner:api-engineer]
- [x] [4.3] [P-1] Add dispatch cases for `read_outlook_inbox` and `read_outlook_calendar` in `execute_tool()` — extract params from `serde_json::Value`, construct `OutlookClient` via `MsGraphUserAuth::try_load_or_prompt()`, call handler [owner:api-engineer]
- [x] [4.4] [P-2] Add dispatch case for `query_ado_work_items` in `execute_tool()` — use existing `AdoClient::from_env()`, extract params with defaults (`ADO_PROJECT`, `assigned_to = "@Me"`, `state = "active"`, `limit = 20`) [owner:api-engineer]
- [x] [4.5] [P-2] Add `ado_work_items` to `ado_tool_definitions()` call and update tool-count assertion in tests if one exists (`// + 3 ado` comment in mod.rs → `// + 4 ado`) [owner:api-engineer]
- [x] [4.6] [P-3] Integration test: `all_tool_definitions()` includes `read_outlook_inbox`, `read_outlook_calendar`, `query_ado_work_items` by name [owner:api-engineer]

## Phase 5 — Verify

- [x] [5.1] `cargo build` passes [owner:api-engineer]
- [x] [5.2] `cargo clippy -- -D warnings` passes (new code clean; pre-existing errors in orchestrator.rs/callbacks.rs not introduced by this spec) [owner:api-engineer]
- [x] [5.3] `cargo test` — all new unit tests pass (250 tests in nv-tools); nv-daemon tests blocked by pre-existing http.rs contact_store errors unrelated to this spec [owner:api-engineer]
- [ ] [5.4] [user] Manual: run `nv auth graph` (or trigger device-code from tool), verify token written to `~/.config/nv/graph-token.json`
- [ ] [5.5] [user] Manual: ask Nova "Show my Outlook inbox" via Telegram, verify formatted email list
- [ ] [5.6] [user] Manual: ask Nova "What's on my calendar today?" via Telegram, verify calendar events
- [ ] [5.7] [user] Manual: ask Nova "What are my active ADO work items on Wholesale Architecture?" via Telegram, verify work item list
