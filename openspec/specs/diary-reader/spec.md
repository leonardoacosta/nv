# diary-reader Specification

## Purpose
TBD - created by archiving change add-diary-system. Update Purpose after archive.
## Requirements
### Requirement: getEntriesByDate queries diary rows for a single calendar day
The reader module SHALL export `getEntriesByDate(date: string, limit?: number): Promise<DiaryEntryItem[]>` that queries rows where `created_at::date = date`, ordered descending, with a default limit of 50. The returned shape MUST be compatible with `DiaryEntryItem` from `apps/dashboard/types/api.ts`.

#### Scenario: Entries exist for the requested date

Given 5 diary rows with `created_at` timestamps on 2026-03-25
When `getEntriesByDate("2026-03-25")` is called
Then the function returns an array of 5 `DiaryEntryItem` objects ordered by `created_at` descending

#### Scenario: No entries for the requested date

Given no diary rows on 2026-03-20
When `getEntriesByDate("2026-03-20")` is called
Then the function returns an empty array

#### Scenario: Limit is respected

Given 100 diary rows on 2026-03-25
When `getEntriesByDate("2026-03-25", 10)` is called
Then only the 10 most recent entries are returned

#### Scenario: Default limit of 50

Given 80 diary rows on 2026-03-25
When `getEntriesByDate("2026-03-25")` is called with no limit argument
Then 50 entries are returned

### Requirement: getEntriesByDateRange queries diary rows across a date span
The reader module SHALL export `getEntriesByDateRange(from: string, to: string): Promise<DiaryEntryItem[]>` that queries rows where `created_at::date` falls within the inclusive range `[from, to]`, ordered descending.

#### Scenario: Range spans two days

Given entries on 2026-03-24 and 2026-03-25
When `getEntriesByDateRange("2026-03-24", "2026-03-25")` is called
Then entries from both days are returned, ordered by `created_at` descending

#### Scenario: Single-day range behaves like getEntriesByDate

Given 3 entries on 2026-03-25
When `getEntriesByDateRange("2026-03-25", "2026-03-25")` is called
Then the same 3 entries are returned

### Requirement: Drizzle rows map to DiaryEntryItem interface
Each Drizzle row returned from the `diary` table MUST be projected to the `DiaryEntryItem` interface: `time` = ISO string of `created_at`; all other fields mapped 1:1 by column name.

#### Scenario: Row fields map to interface fields

Given a diary row with `created_at`, `trigger_type`, `trigger_source`, `channel`, `slug`, `tools_used`, `response_latency_ms`, `tokens_in`, `tokens_out`
When the row is mapped
Then the resulting `DiaryEntryItem` has `time` equal to the ISO string of `created_at` and all other named fields present with their column values

