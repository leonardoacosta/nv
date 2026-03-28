# Prompt Preview Drawer

## ADDED Requirements

### Requirement: Slide-out prompt preview drawer

The dashboard SHALL provide a slide-out drawer component that opens from each automation card
(Watcher, Briefing) showing the full assembled system prompt with context sections, channel
indicators, and filter controls. The drawer MUST be 560px on desktop, full-width on mobile, slide
in from the right with a backdrop overlay, and be lazy-loaded.

#### Scenario: User opens prompt preview for Briefing

Given the user clicks "Preview Prompt" on the BriefingCard
When the drawer opens and fetches `automation.previewContext` with `type: "briefing"`
Then the drawer displays: the static briefing system prompt preamble in a read-only code block,
the custom user prompt (highlighted), and gathered context sections (obligations summary, memory
topics, messages grouped by channel, stats), plus an "assembled at" timestamp.

#### Scenario: User opens prompt preview for Watcher

Given the user clicks "Preview Prompt" on the WatcherCard
When the drawer opens and fetches `automation.previewContext` with `type: "watcher"`
Then the drawer displays: the watcher scan description, the custom user prompt, and obligation
context (overdue, stale, approaching items with counts), plus channel source indicators.

### Requirement: Channel source indicators

The drawer SHALL display horizontal pills for each known channel (Telegram, Discord, Teams, Email,
Dashboard). Active channels with messages in the context window MUST show a highlighted border and
message count badge. Inactive channels MUST appear dimmed with zero count.

#### Scenario: Mixed active and inactive channels

Given the context contains messages from Telegram (15) and Dashboard (3) but none from Discord
When the drawer renders channel pills
Then Telegram shows "15" with active styling, Dashboard shows "3" with active styling, and
Discord/Teams/Email show dimmed with "0" count.

### Requirement: Filter controls

The drawer header SHALL contain filter controls for time range (1h, 6h, 12h, 24h, 7d), obligation
status (multi-select chips), and channel inclusion (checkboxes). Filters MUST be applied
client-side with 300ms debounce and SHALL NOT be persisted.

#### Scenario: User narrows time range

Given the drawer is open showing 24h of messages (50 messages)
When the user selects "6h" from the time range dropdown
Then the messages section re-filters to show only messages from the last 6 hours, channel counts
update accordingly, and the filter applies with a 300ms debounce.

### Requirement: Context summary bar

The dashboard SHALL render a compact summary bar above each automation card showing: active
obligations count (N open, M in progress), memory topics loaded, messages in context (by channel),
and last assembly timestamp. The bar MUST refresh on the 30-second polling cycle.

#### Scenario: Summary bar shows obligation breakdown

Given there are 5 open obligations and 2 in-progress obligations
When the automations page loads and the context summary refreshes
Then the summary bar above the WatcherCard shows "5 open, 2 in progress" for obligations, the
memory topic count, the message count, and the last assembly time.
