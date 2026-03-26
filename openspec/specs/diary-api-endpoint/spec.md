# diary-api-endpoint Specification

## Purpose
TBD - created by archiving change add-diary-system. Update Purpose after archive.
## Requirements
### Requirement: GET /api/diary returns DiaryGetResponse JSON
The HTTP handler for `GET /api/diary` SHALL accept optional query params `date` (YYYY-MM-DD, default today) and `limit` (integer, default 50), call `getEntriesByDate`, and respond with `{ date, entries: DiaryEntryItem[], total }` matching `DiaryGetResponse` from `apps/dashboard/types/api.ts`. An empty date MUST return 200 with `entries: []`. A malformed date MUST return 400.

#### Scenario: Default request — today's entries

Given 7 entries today
When `GET /api/diary` is called with no query params
Then the response is `{ date: "<today>", entries: [...7 items], total: 7 }` with HTTP 200

#### Scenario: Filtered by date param

Given 3 entries on 2026-03-20
When `GET /api/diary?date=2026-03-20` is called
Then the response contains `date: "2026-03-20"` and 3 entries with HTTP 200

#### Scenario: Filtered by limit param

Given 80 entries today
When `GET /api/diary?limit=20` is called
Then `entries` contains 20 items and `total` is 20

#### Scenario: No entries for date returns 200 with empty array

Given no entries on 2026-01-01
When `GET /api/diary?date=2026-01-01` is called
Then the response is `{ date: "2026-01-01", entries: [], total: 0 }` with HTTP 200

#### Scenario: Malformed date param returns 400

Given `date=not-a-date`
When `GET /api/diary?date=not-a-date` is called
Then HTTP status is 400 and body contains `{ error: "Invalid date format. Expected YYYY-MM-DD." }`

