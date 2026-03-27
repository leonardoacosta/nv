# Spec: cleanup-diary-ui

## Parent Change
`cleanup-diary-page`

## MODIFIED Requirements

### Requirement: DiaryGetResponse MUST include day-level aggregate fields

The API route SHALL compute `distinct_channels` and `last_interaction_at` from the queried rows and
include them in the JSON response alongside the existing `date`, `entries`, and `total` fields.

#### Scenario: Day with entries from multiple channels
- **Given** the diary table has entries for 2026-03-27 from channels "telegram", "discord", "telegram"
- **When** GET /api/diary?date=2026-03-27 is called
- **Then** the response includes `distinct_channels: 2` and `last_interaction_at` set to the ISO
  timestamp of the most recent entry

#### Scenario: Day with no entries
- **Given** the diary table has no entries for 2026-03-28
- **When** GET /api/diary?date=2026-03-28 is called
- **Then** the response includes `distinct_channels: 0` and `last_interaction_at: null`

### Requirement: Stats bar MUST show user-meaningful metrics instead of developer metrics

The stats bar SHALL display three metrics: entry count for the day, distinct active channels, and
relative time since last interaction. Token count and average latency stats MUST be removed from the
page-level display.

#### Scenario: Stats bar with active day
- **Given** the API returns 12 entries, 3 distinct channels, and last interaction 5 minutes ago
- **When** the diary page renders
- **Then** the stats bar shows "12" for entries, "3" for active channels, and "5m ago" for last
  interaction -- no token or latency stats are visible

### Requirement: Diary entries MUST render as compact expandable rows

Each entry SHALL render as a single-line compact row with monospace HH:MM:SS timestamp, color-coded
channel icon (from `PLATFORM_BRAND`), trigger type badge, and truncated one-line summary. Clicking
the row MUST toggle an expanded section showing tool pills, full content in a monospace code block,
and token/latency metadata.

#### Scenario: Collapsed entry row
- **Given** a diary entry with time "2026-03-27T14:32:15Z", channel "telegram", trigger_type
  "message", and result_summary "Checked Jira board and closed OO-142"
- **When** the entry renders in collapsed state
- **Then** it displays "14:32:15" in monospace, a Telegram-colored icon, a "message" badge, and the
  summary text truncated to one line

#### Scenario: Expanded entry row
- **Given** the user clicks a collapsed entry row
- **When** the row expands
- **Then** it reveals tool pills, the full `result_summary` in a scrollable monospace code block, and
  a metadata line showing latency and token counts

### Requirement: Entries MUST be grouped under a day header

A date header SHALL appear above the entry list showing the contextual day label ("Today",
"Yesterday", or the full formatted date). The header MUST provide visual grouping consistent with
the date navigation.

#### Scenario: Viewing today's entries
- **Given** the selected date is today
- **When** the diary page renders with entries
- **Then** a "Today" header appears above the entry list with the full date as a subtitle

#### Scenario: Viewing a past date
- **Given** the selected date is 2026-03-24
- **When** the diary page renders
- **Then** a "Monday, March 24, 2026" header appears above the entry list

### Requirement: Raw content MUST be rendered in collapsible code block

The full `result_summary` text within the expanded entry view SHALL render inside a `<pre>` / code
block with monospace font, horizontal scroll for long lines, and a subtle background. This MUST
replace the current inline paragraph rendering.

#### Scenario: Multi-line content display
- **Given** an entry whose `result_summary` contains 5 lines of text
- **When** the entry is expanded
- **Then** the content appears in a monospace code block with `overflow-x-auto` and does not wrap
  beyond the container width

### Requirement: Page title MUST be simplified to "Activity Log"

The page heading SHALL read "Activity Log" with subtitle "Nova's interaction history". The `BookOpen`
icon MUST be retained.

#### Scenario: Page title display
- **Given** the diary page loads
- **When** the header renders
- **Then** it shows "Activity Log" as the h1 and "Nova's interaction history" as the subtitle
