# Proposal: MS Graph CLI Tools

## Change ID
`ms-graph-cli-tools`

## Summary

Rust daemon tools for Outlook email, Outlook calendar, and expanded Azure DevOps (work items,
plus the existing pipelines/builds). Adds a `MsGraphClient` with dual auth: device-code flow
for interactive/CLI use (delegated permissions via cached token) and client-credentials for
daemon background use (application permissions). New tool modules: `tools/outlook.rs` and
expanded `tools/ado.rs`. Five net-new tools: `read_outlook_inbox`, `read_outlook_calendar`,
`query_ado_work_items`, plus enhancing ADO client to support AAD token auth alongside existing
PAT auth.

## Context

- Extends: `crates/nv-daemon/src/tools/` (new modules), `crates/nv-daemon/src/tools/mod.rs`
  (registration + dispatch), `crates/nv-core/src/config.rs` (config structs)
- Reuses: `channels/teams/oauth.rs` `MsGraphAuth` for daemon client-credentials auth
- Related: `tools/teams.rs` pattern (builds teams client from secrets, same credential env vars),
  existing `tools/ado.rs` (PAT auth pattern to extend)
- Script reference: `/home/nyaptor/dev/ws/scripts/graph-outlook.ps1` — Graph API endpoints and
  field selections; `/home/nyaptor/dev/ws/scripts/ado` — ADO SSH proxy pattern (daemon uses
  direct REST instead)
- PRD ref: Phase 3 — Data & Integrations, Wave 7
- Beads epic: `nv-kiti`

## Motivation

The existing scripts (`outlook`, `graph-outlook.ps1`, `ado`) work only from the homelab by
proxying through the cloudpc Windows VM via SSH. Nova runs headless as a systemd daemon and
needs native Graph API access:

1. **Outlook inbox** — "What emails came in today from Sarah?" without opening a browser.
2. **Outlook calendar** — "What's on my calendar this afternoon?" — covers meetings the
   Google Calendar tool cannot see (corporate Exchange calendar is O365-only).
3. **ADO work items** — "What are my active work items on Wholesale Architecture?" extends
   the existing `ado_pipelines`/`ado_builds` tools with the work-item tier.

Both Outlook and Calendar require delegated permissions (Mail.Read, Calendars.Read) which
client-credentials cannot acquire. The spec adds device-code flow so Nova can authenticate
interactively on first use and cache the refresh token, while Teams and ADO continue using
client-credentials.

## Architecture: Dual Auth Model

### Client-Credentials (daemon background)

Used by: Teams channel, `teams_*` tools, `ado_*` tools (new AAD path).

- Token source: `MsGraphAuth` (existing, `channels/teams/oauth.rs`)
- Env vars: `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, `MS_GRAPH_TENANT_ID`
- Permissions: application-level (granted in Azure AD app registration)
- Token lifetime: 1 hour, auto-refreshed in memory

### Device-Code Flow (delegated — interactive first use)

Used by: `read_outlook_inbox`, `read_outlook_calendar`.

- Token source: new `MsGraphUserAuth` in `channels/teams/oauth.rs` (or a sibling module)
- Env vars: `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_TENANT_ID` (same app, no secret needed for
  public/device-code clients — uses `MS_GRAPH_CLIENT_SECRET` if confidential client)
- Token cache: `~/.config/nv/graph-token.json` — access token + refresh token + expiry
  (mirrors the Windows `~/.graph-token.json` pattern from `graph-teams.ps1`)
- Permissions: delegated (Mail.Read, Calendars.Read, offline_access)
- Refresh: silent refresh on expiry via stored refresh_token; re-prompt device code only
  if refresh fails
- Scopes: `https://graph.microsoft.com/Mail.Read https://graph.microsoft.com/Calendars.Read offline_access`

### MsGraphClient (new shared wrapper)

A new `MsGraphClient` struct in `crates/nv-daemon/src/tools/ms_graph.rs` that:
- Holds an `Arc<MsGraphAuth>` for daemon (client-creds) calls OR a `MsGraphUserAuth`
  token for user-delegated calls
- Provides `get(url)` helper that injects the correct Bearer token and handles 429 retry
- Both Outlook tools construct a `MsGraphClient` using the user auth path
- The existing TeamsClient and tools remain unchanged (they use their own auth directly)

## Requirements

### Req-1: MsGraphUserAuth — Device-Code Token Manager

New struct `MsGraphUserAuth` in `crates/nv-daemon/src/channels/teams/oauth.rs`
(or a new `crates/nv-daemon/src/ms_graph_user_auth.rs`).

- `from_cache(path: &Path) -> Option<Self>` — load cached token JSON, return None if missing or
  expired with no refresh token
- `device_code_flow(client_id, tenant_id, scopes) -> Result<Self>` — interactive device-code
  auth: POST devicecode endpoint, print user code + verification URL to stderr, poll for token
- `get_token(&self) -> Result<String>` — return cached token or attempt silent refresh via
  refresh_token; if refresh fails, return `Err` with message prompting user to re-authenticate
  using `nv auth graph`
- `save(&self, path: &Path) -> Result<()>` — persist access_token, refresh_token, expiry as JSON
- Token file location: `~/.config/nv/graph-token.json` (configurable via `NV_GRAPH_TOKEN_PATH`)

### Req-2: read_outlook_inbox Tool

```
read_outlook_inbox(folder?: String, count?: u32, unread_only?: bool) -> String
```

Calls `GET /me/mailFolders/{folder}/messages` (defaults to `Inbox`) with `$select`,
`$orderby=receivedDateTime desc`, `$top={count}` (default 10, max 25).

Fields to select: `id,subject,from,receivedDateTime,isRead,hasAttachments,importance,bodyPreview`

Output format (matches `graph-outlook.ps1` inbox view):
```
Inbox — 10 messages (3 unread)
* [2026-03-25T14:30:00Z] Sarah Martinez [+] — Meeting follow-up
  Preview: Please find attached the action items from today's...
  [2026-03-25T12:00:00Z] No-Reply@company.com — Build notification
  Preview: Build #4521 completed successfully on main...
```
(`*` = unread, `[+]` = has attachment)

Folder resolution: if `folder` param is not `Inbox`, fetch folder list and match by
`displayName` (case-insensitive) to get folder ID, then query messages in that folder.

### Req-3: read_outlook_calendar Tool

```
read_outlook_calendar(days_ahead?: u32, max_events?: u32) -> String
```

Calls `GET /me/calendarView` with `startDateTime` (now in UTC) and `endDateTime`
(now + days_ahead days, default 1). Returns up to `max_events` events (default 10, max 25).

Fields: `subject,start,end,organizer,attendees,location,isAllDay,isCancelled,bodyPreview`

Output format:
```
Calendar — 2026-03-25 (3 events)
[09:00–09:30] Standup (Teams) — All-hands standup
  Organizer: sarah@company.com | 8 attendees
[14:00–15:00] Architecture Review (Room 302)
  Organizer: leo@company.com | 3 attendees
[All Day] Company Holiday
```

Timezone: display in user's local timezone if available, otherwise UTC with `(UTC)` suffix.
Config value `[calendar].timezone` (existing `default_timezone()`) used for display.

### Req-4: query_ado_work_items Tool

```
query_ado_work_items(project?: String, assigned_to?: String, state?: String, limit?: u32) -> String
```

Calls ADO WIQL endpoint:
`POST /{project}/_apis/wit/wiql?api-version=7.1`

Body:
```json
{
  "query": "SELECT [System.Id], [System.Title], [System.State], [System.WorkItemType], [System.AssignedTo] FROM WorkItems WHERE [System.TeamProject] = '{project}' AND [System.State] <> 'Closed' AND [System.AssignedTo] = @Me ORDER BY [System.ChangedDate] DESC"
}
```

Then batch-fetch work item details via:
`GET /_apis/wit/workitems?ids={comma-list}&fields=System.Id,System.Title,System.State,System.WorkItemType,System.AssignedTo,System.ChangedDate&api-version=7.1`

Parameters:
- `project`: ADO project name (default: `ADO_PROJECT` env var)
- `assigned_to`: filter by assignee (default: `@Me` — current user via PAT identity)
- `state`: filter (default: exclude Closed), accepts `Active`, `New`, `Resolved`, `all`
- `limit`: max items returned (default 20, max 50)

Output format:
```
Work Items — Wholesale Architecture (5 active)
[#12345] Bug — Login timeout on Azure AD redirect [Active]
  Assigned: Leonardo Acosta | Changed: 2026-03-24
[#12301] Task — Migrate event schema to v2 [New]
  Assigned: Leonardo Acosta | Changed: 2026-03-23
```

Auth: uses existing `AdoClient` PAT auth (same as `ado_pipelines`/`ado_builds`).

### Req-5: Extend AdoClient with AAD Token Support (Optional Enhancement)

The existing `AdoClient::from_env()` uses PAT. The `ado` shell script uses AAD tokens
via cloudpc proxy. For parity, add `AdoClient::from_aad_token(token: &str)` that accepts
an AAD Bearer token instead of a PAT, using `Authorization: Bearer {token}`. This is
optional for Wave 7 but enables future AAD-only ADO orgs.

### Req-6: Tool Registration

Register new tools in `tools/mod.rs`:
- `read_outlook_inbox` — via `outlook::read_outlook_inbox()`
- `read_outlook_calendar` — via `outlook::read_outlook_calendar()`
- `query_ado_work_items` — via `ado::query_ado_work_items()`

All tools fail gracefully if credentials are not configured, returning a descriptive
"not configured" message rather than panicking.

### Req-7: Config Extensions

Add to `config.rs` if not already present (or confirm existing fields suffice):
- `NV_GRAPH_TOKEN_PATH` — path override for device-code token cache file
- `ADO_PROJECT` — default ADO project name (already handled via env var in ado.rs)

No new TOML config structs required — secrets come from env vars.

### Req-8: nv-cli Auth Command (Optional, Deferred)

A future `nv auth graph` CLI subcommand that triggers the device-code flow interactively,
saves the token, and prints "Graph API authenticated for <user>@<tenant>". This is
**deferred** to a follow-up spec — the daemon tools will print instructions to stderr
if no cached token is found, directing the user to run `nv auth graph`.

## Scope

**IN:**
- `MsGraphUserAuth` struct with device-code + refresh-token cache
- `tools/outlook.rs` — `read_outlook_inbox`, `read_outlook_calendar`
- `tools/ado.rs` — `query_ado_work_items` added to existing module
- Tool registration in `tools/mod.rs`
- Graceful "not configured" fallback for all three new tools

**OUT:**
- Sending email or creating calendar events (read-only scope)
- Creating/updating ADO work items (read-only)
- `nv auth graph` CLI subcommand (deferred)
- Attachment download (body preview only)
- Pagination beyond the `count`/`limit` cap

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/channels/teams/oauth.rs` | Add `MsGraphUserAuth` with device-code flow and token cache |
| `crates/nv-daemon/src/tools/outlook.rs` | New: `read_outlook_inbox`, `read_outlook_calendar`, formatting helpers |
| `crates/nv-daemon/src/tools/ado.rs` | Add `query_ado_work_items`, WIQL query, batch work item fetch |
| `crates/nv-daemon/src/tools/mod.rs` | Register 3 new tools, add `mod outlook;`, extend ado dispatch |
| `crates/nv-core/src/config.rs` | No change expected (env vars sufficient) |
| `config/nv.toml` or `.env` | Document `NV_GRAPH_TOKEN_PATH`, confirm `ADO_PROJECT` |

## Risks

| Risk | Mitigation |
|------|-----------|
| Device-code token requires interactive first-use | Print clear instructions to stderr. Future `nv auth graph` command handles this. |
| Mail.Read / Calendars.Read require delegated permissions | Cannot be granted to client-credentials app. Device-code flow is the correct solution. |
| `@Me` WIQL filter depends on PAT identity | Test with known `assigned_to` param as fallback. Document PAT scope requirement. |
| ADO WIQL may return 0 results for `@Me` if PAT is service account | Allow `assigned_to` override. |
| Token cache file permissions | Write with mode 0o600. |
| Tenant-specific Graph endpoints | All endpoints use `/me/` which is tenant-agnostic with delegated tokens. |
| Corporate proxy / conditional access policy | Teams client-creds already works, confirming Graph API is reachable from homelab. |
