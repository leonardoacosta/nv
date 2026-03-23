# Proposal: Google Calendar Integration

## Change ID
`add-calendar-integration`

## Summary

Add read-only Google Calendar tools so Nova can be meeting-aware. Three tools — `calendar_today`, `calendar_upcoming`, and `calendar_next` — query Google Calendar API v3 via REST. Calendar events are also injected into the proactive digest context so the morning summary includes schedule awareness.

## Context
- Extends: `crates/nv-core/src/config.rs` (add CalendarConfig), `crates/nv-daemon/src/tools.rs` (tool registration + dispatch), `crates/nv-daemon/src/digest/gather.rs` (inject calendar context into digest)
- Related: Other tool modules (`posthog_tools.rs`, `vercel_tools.rs`, etc.) for HTTP API client patterns. `digest/gather.rs` for parallel data gathering with partial failure tolerance. `config.rs` for `[section]` config + `Secrets` env-var pattern.
- Depends on: nothing — standalone addition

## Motivation

Nova's proactive digest fires daily with project health summaries (Jira issues, Nexus sessions, memory), but has zero schedule awareness. The morning digest doesn't know about meetings, deadlines, or time-blocked focus periods. This means Nova can't say "You have 3 meetings today, first one at 9am" or factor meetings into priority recommendations.

Calendar awareness is table-stakes for a proactive assistant. Without it, Nova's digest is incomplete and the user must check their calendar separately. Adding calendar tools also enables ad-hoc queries ("What's my next meeting?", "What's on my schedule Thursday?") through the existing tool framework.

## Requirements

### Req-1: CalendarConfig in nv.toml

Add an optional `[calendar]` section to Config:

```toml
[calendar]
calendar_id = "primary"          # default "primary"
```

- `calendar_id`: Google Calendar ID to query (default `"primary"` for the user's main calendar)
- The section is optional — if absent, calendar tools return "Calendar not configured" and digest skips calendar context

Auth credentials are stored in `Secrets` via environment variables (Req-2), not in the config file.

### Req-2: Google Calendar Auth via Secrets

Add to the `Secrets` struct:

- `google_calendar_credentials`: `Option<String>` from `GOOGLE_CALENDAR_CREDENTIALS` env var

The credentials value is a base64-encoded JSON service account key or a JSON blob containing an OAuth2 refresh token, client ID, and client secret. The calendar module handles decoding and token refresh.

Token refresh flow:
1. On first API call, decode credentials and obtain an access token
2. Cache the access token in memory with its expiry
3. On subsequent calls, reuse cached token if not expired
4. If expired, refresh using the refresh token / service account assertion
5. If refresh fails, return error (don't crash — partial failure tolerance)

### Req-3: `calendar_today` Tool

Get today's events from Google Calendar API v3.

- Endpoint: `GET https://www.googleapis.com/calendar/v3/calendars/{calendarId}/events`
- Parameters: `timeMin` = start of today (midnight local), `timeMax` = end of today, `singleEvents=true`, `orderBy=startTime`
- Returns formatted text: each event with title, start time, end time, attendees (names), location/meeting link
- Empty day: "No events scheduled for today."
- Tool schema: `calendar_today` with no required parameters (uses configured calendar_id)

Format example:
```
Today's schedule (3 events):
  09:00-09:30  Team standup (Google Meet: https://meet.google.com/...)
    Attendees: Alice, Bob, Charlie
  11:00-12:00  Design review
    Location: Room 4B
  14:00-15:00  1:1 with manager
    Attendees: Manager Name
```

### Req-4: `calendar_upcoming` Tool

Get events for the next N days (default 7).

- Same endpoint, but `timeMin` = now, `timeMax` = now + N days
- Parameter: `days` (integer, default 7, max 30)
- Returns formatted text grouped by day
- Tool schema: `calendar_upcoming` with optional `days` parameter

### Req-5: `calendar_next` Tool

Get the next upcoming event (quick check).

- Same endpoint, `timeMin` = now, `maxResults=1`, `singleEvents=true`, `orderBy=startTime`
- Returns a single event's details, or "No upcoming events."
- Tool schema: `calendar_next` with no required parameters

### Req-6: Digest Calendar Context Injection

Add calendar data to the proactive digest pipeline:

1. Add `gather_calendar()` to `digest/gather.rs` that calls the calendar_today logic
2. Add `calendar_events: Vec<CalendarDigestEvent>` to `DigestContext`
3. Join `gather_calendar()` into the existing `tokio::join!()` alongside Jira/memory/nexus
4. Add `[Calendar]` section to `format_context_for_prompt()` output
5. Partial failure: if calendar API is down, log warning, add to `errors`, digest continues without calendar

The `CalendarDigestEvent` struct needs: `title`, `start_time`, `end_time`, `attendees_count`, `has_meeting_link`.

### Req-7: HTTP Client Module

Create `crates/nv-daemon/src/calendar_tools.rs` following the existing tool module pattern:

- Module-level doc comment describing purpose and auth
- `const CALENDAR_TIMEOUT: Duration = Duration::from_secs(15)`
- Response type structs (deserialized from Google Calendar API JSON)
- Auth helper: decode credentials, obtain/refresh access token
- Three public async functions: `calendar_today()`, `calendar_upcoming()`, `calendar_next()`
- Each returns `Result<String>` with formatted output
- Error mapping for HTTP status codes (401 = credentials invalid, 403 = calendar access denied, 404 = calendar not found, 429 = rate limited)

## Scope
- **IN**: CalendarConfig, Secrets field, Google Calendar API v3 REST client, three read-only tools (`calendar_today`, `calendar_upcoming`, `calendar_next`), digest context injection, token caching, partial failure tolerance
- **OUT**: Event creation/modification/deletion, multiple calendar support, webhook-based push notifications, CalDAV, iCal import, recurring event expansion logic (Google API handles this with `singleEvents=true`), calendar-based notification scheduling, Outlook/Exchange calendars

## Impact
| Area | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `CalendarConfig` struct, add `calendar: Option<CalendarConfig>` to `Config`, add `google_calendar_credentials` to `Secrets` |
| `crates/nv-daemon/src/calendar_tools.rs` (new) | Google Calendar API v3 client: auth, token refresh, three query functions, response formatting |
| `crates/nv-daemon/src/tools.rs` | Add `calendar_tool_definitions()`, register in `register_tools()`, add dispatch arms for `calendar_today`, `calendar_upcoming`, `calendar_next` |
| `crates/nv-daemon/src/digest/gather.rs` | Add `CalendarDigestEvent`, `gather_calendar()`, extend `DigestContext` and `format_context_for_prompt()` |
| `crates/nv-daemon/src/main.rs` | Add `mod calendar_tools;` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Google Calendar API requires OAuth2 token refresh | Cache access token with expiry. Refresh on 401. Service accounts don't need user interaction. |
| Calendar API down shouldn't break digest | Partial failure tolerance — same pattern as Jira/Nexus in `gather_context()`. Calendar failure adds to `errors` vec, digest continues. |
| Service account may not have calendar access | Clear error message: "Calendar access denied (403) — ensure service account has read access to calendar {id}". |
| Token refresh latency on first call | First call may take ~500ms extra for token exchange. Subsequent calls reuse cached token. |
| Large number of events (busy day) | Google API returns max 250 events by default. For `calendar_today`, this is sufficient. For `calendar_upcoming` with 30 days, set `maxResults=100` to cap response size. |
| Credentials format complexity (service account vs OAuth refresh token) | Start with service account JSON key (simpler, no user interaction). Document OAuth refresh token as future alternative. |
