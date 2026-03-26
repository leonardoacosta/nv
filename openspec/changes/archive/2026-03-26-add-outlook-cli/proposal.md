# Proposal: Add Outlook CLI Tools

## Change ID
`add-outlook-cli`

## Summary

Add native MS Graph Outlook tools to `nv-tools` — `outlook_inbox`, `outlook_calendar`, and
`outlook_read_email` — using device-code flow with a persistent token cache. Replaces the
CloudPC SSH bridge in `nv-daemon/src/tools/outlook.rs`.

## Context

- Extends: `crates/nv-tools/src/tools/` (new `outlook.rs`), `crates/nv-tools/src/tools/mod.rs`
  (registration), `crates/nv-tools/src/dispatch.rs` (dispatch cases + tool definitions)
- Replaces: `crates/nv-daemon/src/tools/outlook.rs` (SSH/CloudPC bridge — two stub handlers
  that shell out to `graph-outlook.ps1`; keep the file but redirect dispatch to nv-tools once
  this lands)
- Reuses auth pattern: `crates/nv-daemon/src/channels/teams/oauth.rs` `MsGraphUserAuth` —
  device-code flow, from_cache/save, token path `~/.config/nv/graph-token.json`. The new
  nv-tools implementation writes to `~/.config/nv/graph-user-token.json` to avoid clobbering
  the daemon's cache.
- Related: `2026-03-26-ms-graph-cli-tools` (archived) — designed `MsGraphUserAuth` for daemon;
  this spec ports a minimal self-contained version into nv-tools.
- Stack: Rust, Phase 2 — Tool Wrappers, Wave 4

## Motivation

`nv-daemon/src/tools/outlook.rs` currently proxies through the CloudPC Windows VM via SSH,
running a PowerShell script that manages its own token. This approach:

1. Requires the CloudPC to be on and reachable.
2. Does not support `read_email <id>` (fetch full body of a specific message).
3. Passes no parameters — count, folder, and date-range are ignored.

`nv-daemon` already has `MsGraphUserAuth` with device-code flow. `nv-tools` should have an
equivalent self-contained implementation so the stateless MCP binary can call Graph directly
without daemon coupling. The token cache is written to `~/.config/nv/graph-user-token.json`
(separate path from the daemon's `graph-token.json`) so the two caches co-exist without
conflicts during the migration period.

## Requirements

### Req-1: GraphUserAuth — standalone token manager in nv-tools

Add `GraphUserAuth` to `crates/nv-tools/src/tools/outlook.rs`.

- `GraphUserAuth::from_env_or_cache() -> Result<Self>` — reads `MS_GRAPH_CLIENT_ID` and
  `MS_GRAPH_TENANT_ID` from environment; loads token from `NV_GRAPH_USER_TOKEN_PATH` (default
  `~/.config/nv/graph-user-token.json`); if missing/expired-without-refresh, runs device-code
  flow for scopes `Mail.Read Calendars.Read offline_access`, then saves the acquired token.
- `GraphUserAuth::get_token(&self) -> Result<String>` — returns cached access token if valid
  (60-second buffer), otherwise silently refreshes via `refresh_token` grant, saves updated
  token, returns new access token; errors with actionable message if refresh fails.
- Token cache format (JSON at path): `{access_token, refresh_token, expires_at_unix, client_id, tenant_id}`
  written with permissions `0o600`.
- Device-code flow prints `user_code` and `verification_uri` to stderr; polls token endpoint
  every `interval` seconds; times out after 300 seconds.

#### Scenario: first-run device-code flow
Given `~/.config/nv/graph-user-token.json` is absent and `MS_GRAPH_CLIENT_ID`/`MS_GRAPH_TENANT_ID`
are set, `GraphUserAuth::from_env_or_cache()` should print device-code instructions to stderr,
block until the user authenticates, save the token, and return Ok.

#### Scenario: cached valid token
Given a valid cached token (expires_at_unix > now + 60), `from_env_or_cache()` returns
immediately without a network call.

#### Scenario: expired token with refresh token
Given an expired access token but valid refresh token, `get_token()` silently exchanges the
refresh token, updates the cache, and returns the new access token.

### Req-2: outlook_inbox tool

`outlook_inbox(auth: &GraphUserAuth, folder: Option<&str>, count: u32, unread_only: bool) -> Result<String>`

- Calls `GET /me/mailFolders/{folder}/messages?$select=id,subject,from,receivedDateTime,isRead,hasAttachments,bodyPreview&$orderby=receivedDateTime desc&$top={count}`
- `folder` defaults to `Inbox`; if a custom folder name is given, resolve it via
  `GET /me/mailFolders?$top=25` matched case-insensitively.
- When `unread_only` is true, appends `&$filter=isRead eq false`.
- Output format (one block per message):
  ```
  [1] Subject line here
      From: Name <addr@domain.com> · 2h ago · 📎 (if has_attachments)
      Preview text truncated to ~120 chars
  ```
- Registered as MCP tool `outlook_inbox` with optional params: `folder?: string`,
  `count?: integer (1–25, default 10)`, `unread_only?: boolean (default false)`.

#### Scenario: default inbox fetch
Given valid auth, `outlook_inbox(auth, None, 10, false)` returns up to 10 messages formatted
as numbered blocks.

#### Scenario: unread only
Given `unread_only: true`, the Graph request includes `$filter=isRead eq false`.

### Req-3: outlook_calendar tool

`outlook_calendar(auth: &GraphUserAuth, days_ahead: u32, max_events: u32) -> Result<String>`

- Calls `GET /me/calendarView?startDateTime={now_utc}&endDateTime={now+days_ahead UTC}&$select=subject,start,end,organizer,attendees,location,isAllDay,isCancelled,bodyPreview&$top={max_events}&$orderby=start/dateTime`
- Groups events by day header (`Monday, Mar 25`) when `days_ahead > 1`.
- Per-event format:
  ```
  [HH:MM–HH:MM] Subject
    Organizer · N attendees · Location (if set)
  ```
  All-day events show `[All Day]` instead of time range.
  Cancelled events are marked `[Cancelled]`.
- Registered as MCP tool `outlook_calendar` with optional params: `days_ahead?: integer (1–14,
  default 1)`, `max_events?: integer (1–25, default 10)`.

#### Scenario: today's calendar
Given `days_ahead: 1`, returns events for today only, no day-header grouping.

#### Scenario: multi-day view
Given `days_ahead: 3`, events are grouped under day headers.

### Req-4: outlook_read_email tool

`outlook_read_email(auth: &GraphUserAuth, message_id: &str) -> Result<String>`

- Calls `GET /me/messages/{id}?$select=subject,from,toRecipients,ccRecipients,receivedDateTime,body,hasAttachments,importance`
- Strips HTML from `body.content` using the same regex-strip helper used in `teams.rs`.
- Output format:
  ```
  Subject: Subject line
  From: Name <addr>
  To: addr1, addr2
  Date: March 25, 2026 at 14:32
  ---
  Body text (HTML stripped, whitespace normalised, max 4000 chars)
  ```
- Registered as MCP tool `outlook_read_email` with required param: `message_id: string`.

#### Scenario: valid message id
Given a valid message ID, returns formatted email with subject, headers, and body.

#### Scenario: invalid message id
Given an unknown ID, Graph returns 404; the tool returns `Err("message not found: {id}")`.

### Req-5: Registration in nv-tools

- Add `pub mod outlook;` to `crates/nv-tools/src/tools/mod.rs`.
- Add `outlook::outlook_tool_definitions()` call in `stateless_tool_definitions()` in
  `crates/nv-tools/src/dispatch.rs`.
- Add dispatch arms for `outlook_inbox`, `outlook_calendar`, `outlook_read_email` in
  `dispatch_stateless()` — each arm calls `GraphUserAuth::from_env_or_cache()` and the
  corresponding handler.

#### Scenario: tool list includes all three
`stateless_tool_definitions()` returns a vec that contains `outlook_inbox`,
`outlook_calendar`, and `outlook_read_email` by name.

### Req-6: Cargo dependencies

Add to `crates/nv-tools/Cargo.toml` only if not already present:
- No new dependencies required — `reqwest`, `serde`, `serde_json`, `chrono`, `tokio`, `anyhow`,
  `uuid` are already workspace deps used by nv-tools.

#### Scenario: build is clean
`cargo build -p nv-tools` succeeds with no new compile errors.

## Scope

- **IN**: `GraphUserAuth` struct, three Graph API tool handlers, MCP tool registration,
  dispatch wiring, unit tests for tool definitions and output formatters
- **OUT**: Removal of the CloudPC outlook.rs (keep it; nv-daemon still uses it until a
  separate migration spec); changes to nv-daemon dispatch; nv-cli binary subcommands;
  Teams or ADO tools; any UI/dashboard changes

## Impact

| Area | Change |
|------|--------|
| `crates/nv-tools/src/tools/outlook.rs` | New file (~300 lines) |
| `crates/nv-tools/src/tools/mod.rs` | Add `pub mod outlook;` |
| `crates/nv-tools/src/dispatch.rs` | 3 dispatch arms + extend tool definitions |
| `crates/nv-tools/Cargo.toml` | No new deps |
| `~/.config/nv/graph-user-token.json` | Runtime-created token cache (not in repo) |

## Risks

| Risk | Mitigation |
|------|-----------|
| Device-code flow not available in CI | Guard behind `integration` feature flag (already used in nv-tools); unit tests use fixtures |
| Token cache path collision with daemon | Use separate filename `graph-user-token.json` vs daemon's `graph-token.json` |
| `Mail.Read` / `Calendars.Read` not granted on the Azure app | Document required permissions in code comment; tool returns actionable error if 403 |
| HTML body stripping incomplete | Reuse `strip_html` regex helper from teams.rs (already tested); truncate at 4000 chars |
