# Spec: Categorized Activity Summaries

## MODIFIED Requirements

### Requirement: Replace flat activity feed with grouped summaries

The activity feed SHALL change from a flat chronological list of all events to a grouped view where events are organized by type (messages, obligations, sessions, system). Each group MUST show a summary header with event count and a natural-language summary, followed by the 3 most recent events. Groups SHALL use progressive disclosure -- collapsed by default, expandable to show all events.

#### Scenario: Events are grouped by type

Given the activity feed contains events of types message, obligation, session, and diary
When the dashboard home page renders
Then events are grouped into four categories with summary headers, ordered by most recent event timestamp per group.

#### Scenario: Group summary header shows count and description

Given the messages group contains 8 events (3 inbound from Telegram, 5 outbound)
When the messages group header renders
Then it shows: MessageSquare icon, "Messages" label, "(8)" count badge, and summary text "3 inbound from Telegram, 5 outbound".

#### Scenario: Only 3 recent events shown per collapsed group

Given the obligations group contains 12 events
When the group is in collapsed state
Then only the 3 most recent obligation events are visible below the summary header, with a "Show more" control.

#### Scenario: Expanding a group shows all events

Given the sessions group is collapsed with 20 events
When the user clicks the group header or "Show more"
Then all 20 session events become visible in the same dense row format with severity coloring.

#### Scenario: Only one group expanded at a time

Given the messages group is currently expanded
When the user expands the obligations group
Then the messages group collapses back to header + 3 items and the obligations group shows all its events.

#### Scenario: Empty groups are hidden

Given no diary events exist in the last 24 hours
When the dashboard home page renders
Then no "System" group header or section appears.

## REMOVED Requirements

### Requirement: Category filter pills

The CategoryPills component (All/Messages/Sessions/Obligations/System tabs) is removed. Category navigation is replaced by the grouped view with per-group expand/collapse.

### Requirement: Standalone Recent Conversations section

The RecentConversations section below the activity feed is removed. Recent message grouping is absorbed into the "Messages" category group within the activity summaries.

## ADDED Requirements

### Requirement: WebSocket badge counters

Real-time WebSocket events SHALL no longer prepend raw entries to the feed. Instead, each incoming event MUST increment a per-category "new events" counter displayed as a badge on the relevant group summary header. Clicking the badge SHALL refresh that category's data from the API and reset the counter.

#### Scenario: WebSocket event increments badge

Given the activity summaries are rendered and a new message event arrives via WebSocket
When the event is received
Then the Messages group header shows a "1 new" badge; subsequent events increment the counter.

#### Scenario: Clicking badge refreshes data

Given the Messages group shows a "3 new" badge
When the user clicks the badge
Then the activity feed query is invalidated, fresh data loads, and the badge resets to zero.
