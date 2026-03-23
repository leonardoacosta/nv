# Implementation Tasks

<!-- beads:epic:TBD -->

## Config Layer

- [x] [1.1] [P-1] Add `CalendarConfig` struct to `crates/nv-core/src/config.rs` — `calendar_id: String` with default `"primary"`, make `[calendar]` section optional (`calendar: Option<CalendarConfig>` on `Config`) [owner:api-engineer]
- [x] [1.2] [P-1] Add `google_calendar_credentials: Option<String>` to `Secrets` struct in `crates/nv-core/src/config.rs` — sourced from `GOOGLE_CALENDAR_CREDENTIALS` env var [owner:api-engineer]

## Calendar Client Module

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/calendar_tools.rs` — module doc comment, `CALENDAR_TIMEOUT` const (15s), Google Calendar API v3 response structs (`EventList`, `Event`, `EventDateTime`, `Attendee`) deserialized from JSON [owner:api-engineer]
- [x] [2.2] [P-1] Implement auth helper in `calendar_tools.rs` — decode base64 service account credentials, obtain access token via JWT assertion, cache token in-memory with expiry, refresh on expiration or 401 [owner:api-engineer]
- [x] [2.3] [P-1] Implement `calendar_today()` in `calendar_tools.rs` — query `GET /calendars/{id}/events` with `timeMin`=start of today, `timeMax`=end of today, `singleEvents=true`, `orderBy=startTime`, return formatted schedule text [owner:api-engineer]
- [x] [2.4] [P-1] Implement `calendar_upcoming()` in `calendar_tools.rs` — same endpoint with `timeMin`=now, `timeMax`=now+N days (default 7, max 30), `maxResults=100`, return events grouped by day [owner:api-engineer]
- [x] [2.5] [P-1] Implement `calendar_next()` in `calendar_tools.rs` — same endpoint with `timeMin`=now, `maxResults=1`, return single event details or "No upcoming events." [owner:api-engineer]
- [x] [2.6] [P-1] Add error mapping in `calendar_tools.rs` — 401→credentials invalid, 403→calendar access denied, 404→calendar not found, 429→rate limited, calendar-not-configured guard [owner:api-engineer]

## Tool Registration

- [x] [3.1] [P-1] Add `calendar_tool_definitions()` in `crates/nv-daemon/src/tools.rs` — define `calendar_today` (no params), `calendar_upcoming` (optional `days` integer), `calendar_next` (no params) tool schemas [owner:api-engineer]
- [x] [3.2] [P-1] Register calendar tools in `register_tools()` in `crates/nv-daemon/src/tools.rs` — call `calendar_tool_definitions()` and append to tool list [owner:api-engineer]
- [x] [3.3] [P-1] Add dispatch arms in `crates/nv-daemon/src/tools.rs` — match `"calendar_today"`, `"calendar_upcoming"`, `"calendar_next"` to their respective functions in `calendar_tools` [owner:api-engineer]

## Digest Injection

- [x] [4.1] [P-1] Add `CalendarDigestEvent` struct to `crates/nv-daemon/src/digest/gather.rs` — fields: `title`, `start_time`, `end_time`, `attendees_count`, `has_meeting_link` [owner:api-engineer]
- [x] [4.2] [P-1] Add `calendar_events: Vec<CalendarDigestEvent>` field to `DigestContext` in `crates/nv-daemon/src/digest/gather.rs` [owner:api-engineer]
- [x] [4.3] [P-1] Implement `gather_calendar()` in `crates/nv-daemon/src/digest/gather.rs` — call calendar_today logic, map events to `CalendarDigestEvent`, return `Vec<CalendarDigestEvent>` [owner:api-engineer]
- [x] [4.4] [P-1] Join `gather_calendar()` into existing `tokio::join!()` in `gather_context()` — partial failure tolerance: on error, log warning, push to `errors` vec, continue with empty calendar [owner:api-engineer]
- [x] [4.5] [P-1] Add `[Calendar]` section to `format_context_for_prompt()` in `crates/nv-daemon/src/digest/gather.rs` — render event count, each event with time/title/attendees, skip section if no events and no error [owner:api-engineer]

## Main Wiring

- [x] [5.1] [P-1] Add `mod calendar_tools;` declaration to `crates/nv-daemon/src/main.rs` [owner:api-engineer]

## Verify

- [x] [6.1] `cargo build` passes [owner:api-engineer]
- [x] [6.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [6.3] `cargo test` — existing tests pass [owner:api-engineer]
