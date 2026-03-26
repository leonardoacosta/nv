# Capability: Outlook Graph Tools

## ADDED Requirements

### Requirement: GraphUserAuth — standalone token manager
`nv-tools` SHALL include a `GraphUserAuth` struct that manages delegated MS Graph tokens via
device-code flow and refresh-token rotation. It reads `MS_GRAPH_CLIENT_ID` and
`MS_GRAPH_TENANT_ID` from environment and caches tokens at
`NV_GRAPH_USER_TOKEN_PATH` (default `~/.config/nv/graph-user-token.json`) with `0o600`
file permissions. The cache format is JSON with fields `access_token`, `refresh_token`,
`expires_at_unix`, `client_id`, `tenant_id`.

#### Scenario: first-run device-code flow
Given `~/.config/nv/graph-user-token.json` is absent and env vars are set,
`GraphUserAuth::from_env_or_cache()` prints `user_code` and `verification_uri` to stderr,
polls the token endpoint every `interval` seconds (max 300s), saves the acquired token to
the cache path, and returns `Ok(GraphUserAuth)`.

#### Scenario: cached valid token — no network call
Given a cached token with `expires_at_unix > now + 60`, `from_env_or_cache()` returns
immediately without any HTTP request.

#### Scenario: expired token with refresh token
Given an expired access token but a valid `refresh_token`, `get_token()` exchanges the
refresh token at the Microsoft token endpoint, overwrites the cache file, and returns the
new access token string.

#### Scenario: expired token with no refresh token
Given an expired access token and no `refresh_token`, `get_token()` returns
`Err("Graph user token expired — re-run the tool to trigger device-code flow")`.

### Requirement: outlook_inbox MCP tool
`nv-tools` SHALL expose an `outlook_inbox` MCP tool that fetches recent messages from an
Outlook mail folder via `GET /me/mailFolders/{folder}/messages`. The tool accepts optional
params `folder` (default `Inbox`), `count` (1–25, default 10), and `unread_only` (default
false). Custom folder names are resolved case-insensitively via `GET /me/mailFolders`. When
`unread_only` is true, `$filter=isRead eq false` is appended. Output is a numbered list of
message blocks showing subject, sender, relative time, and body preview.

#### Scenario: default inbox fetch
Given valid auth and no parameters, the tool calls
`GET /me/mailFolders/Inbox/messages?$top=10&$orderby=receivedDateTime desc` and returns a
numbered list of up to 10 message blocks.

#### Scenario: unread only filter applied
Given `unread_only: true`, the Graph request URL contains `$filter=isRead eq false`.

#### Scenario: 401 triggers token refresh and retry
Given Graph returns HTTP 401, the tool calls `get_token()` to refresh and retries the
request once before returning an error.

### Requirement: outlook_calendar MCP tool
`nv-tools` SHALL expose an `outlook_calendar` MCP tool that fetches upcoming calendar events
via `GET /me/calendarView` with RFC3339-bounded time range. Params: `days_ahead` (1–14,
default 1), `max_events` (1–25, default 10). When `days_ahead > 1`, events are grouped under
day headers (`Monday, Mar 25`). All-day events show `[All Day]`; cancelled events are
prefixed `[Cancelled]`. Timed events show `[HH:MM–HH:MM]`.

#### Scenario: today's events — no day header
Given `days_ahead: 1`, the output lists events without day-header grouping.

#### Scenario: multi-day view — day headers present
Given `days_ahead: 3`, events are grouped under day headers for each date in the range.

#### Scenario: all-day event formatted correctly
Given a calendar event with `isAllDay: true`, the time column in the output shows `[All Day]`.

#### Scenario: cancelled event marked in output
Given a calendar event with `isCancelled: true`, the output line is prefixed `[Cancelled]`.

### Requirement: outlook_read_email MCP tool
`nv-tools` SHALL expose an `outlook_read_email` MCP tool that fetches a single Outlook
message by ID via `GET /me/messages/{id}`. Required param: `message_id`. The tool strips
HTML from `body.content` using a regex-based helper and truncates to 4000 chars. Output
format: Subject, From, To, Date header block followed by a `---` separator and body text.

#### Scenario: valid message id — full formatted output
Given a valid `message_id`, the tool returns a string with Subject, From, To, Date headers
and the HTML-stripped body (max 4000 chars).

#### Scenario: 404 response returns actionable error
Given an unknown `message_id` that Graph responds to with 404, the tool returns
`Err("message not found: {id}")`.

### Requirement: Tool registration in nv-tools stateless dispatch
All three Outlook tools SHALL be registered in `stateless_tool_definitions()` and dispatched
in `dispatch_stateless()` inside `crates/nv-tools/src/dispatch.rs`. `pub mod outlook;` is
added to `crates/nv-tools/src/tools/mod.rs`.

#### Scenario: tool list includes all three tools
`stateless_tool_definitions()` returns a `Vec<ToolDefinition>` containing entries named
`outlook_inbox`, `outlook_calendar`, and `outlook_read_email`.

#### Scenario: dispatch routes to correct handler
Calling `dispatch_stateless("outlook_inbox", &args)` invokes `outlook::outlook_inbox` via
`GraphUserAuth::from_env_or_cache()` without panicking.
