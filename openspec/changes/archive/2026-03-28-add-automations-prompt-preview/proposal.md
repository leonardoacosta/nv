# Proposal: Add Automations Prompt Preview

## Change ID
`add-automations-prompt-preview`

## Summary

Add a slide-out drawer to each automation card (Watcher, Briefing) on the `/automations` page that
shows the full assembled system prompt with loaded context summary, channel source indicators,
filter controls, and an info card clarifying the reminders-vs-obligations distinction.

## Context
- Phase: Dashboard UX enhancement (post-`automations-ux-overhaul`)
- Extends:
  - `apps/dashboard/app/automations/page.tsx` -- current 1185-line page with WatcherCard + BriefingCard
  - `packages/api/src/routers/automation.ts` -- `automationRouter` (getAll, getSettings, updateSettings)
  - `packages/api/src/routers/obligation.ts` -- `obligationRouter` (list, stats)
  - `packages/daemon/src/features/briefing/synthesizer.ts` -- `buildBriefingPrompt()` assembles context from obligations, memory, messages, calendar, diary
  - `packages/daemon/src/features/watcher/proactive.ts` -- scans obligations by status (overdue, stale, approaching)
- Related:
  - `add-automations-page` (original implementation)
  - `automations-ux-overhaul` (archived -- restructured into current 2-column grid)
- Key schemas:
  - `packages/db/src/schema/obligations.ts` -- detectedAction, owner, status, priority, sourceChannel, deadline
  - `packages/db/src/schema/reminders.ts` -- message, dueAt, channel, obligationId (FK to obligations)
  - `packages/db/src/schema/memory.ts` -- topic, content, embedding, updatedAt
  - `packages/db/src/schema/messages.ts` -- channel, sender, content, metadata, threadId

## Motivation

The automations page lets users edit custom prompts for the Watcher and Briefing, but provides no
visibility into what Nova actually sees when these automations run. The user has to guess which
channels feed data, how many obligations are active, which memory topics are included, and what the
assembled system prompt looks like. This creates a "black box" problem:

1. **No prompt transparency** -- the custom prompt textarea is just one piece. The full prompt
   includes obligations, memory entries, recent messages grouped by channel, calendar data, and
   diary entries. Users cannot see this assembled view.

2. **No channel visibility** -- messages are ingested from Telegram, Discord, and Teams but the
   automations page does not show which channels contribute to each automation's context window.

3. **No context summary** -- users cannot see how many obligations, memory topics, or messages are
   loaded into the prompt. This makes it impossible to tune the custom prompt effectively.

4. **Reminders vs obligations confusion** -- the schema correctly distinguishes obligations
   (detected commitments with lifecycle: open -> in_progress -> proposed_done -> done) from
   reminders (one-shot alerts, optionally linked to obligations via FK). This distinction is not
   communicated anywhere in the UI, leading to confusion about what each section shows.

## Requirements

### Req-1: Prompt Preview Drawer

Add a slide-out drawer component that opens from each automation card (Watcher, Briefing). The
drawer shows the full assembled system prompt as it would be sent to the LLM, including:

- The static system prompt preamble (read-only)
- The custom user prompt (editable, synced with the textarea on the card)
- The gathered context sections: obligations summary, memory topics, recent messages by channel,
  calendar preview, diary activity
- A timestamp showing when this context was last assembled

The drawer opens via a "Preview Prompt" button on each card, positioned next to the existing
"Custom Prompt" collapsible. It slides in from the right edge, overlaying the page content with a
backdrop. Width: 560px on desktop, full-width on mobile.

### Req-2: Context Preview API Endpoint

Add a new tRPC procedure `automation.previewContext` that returns the assembled context for a given
automation type without triggering the actual automation run. This procedure:

- Accepts `{ type: "watcher" | "briefing" }` input
- Gathers context using the same logic as the daemon's `gatherContext()`: queries obligations
  (pending/in_progress, limit 20), memory (latest 10 topics), messages (latest 20, grouped by
  channel), and obligation stats
- Returns structured sections:
  - `obligations`: count by status, list of active items (id, action, owner, status, priority)
  - `memory`: list of topic names with content preview (first 100 chars)
  - `messages`: grouped by channel with count and latest message preview per channel
  - `channels`: list of distinct channels with message counts in the time window
  - `stats`: total obligations, active reminders count, memory topics count

### Req-3: Channel Source Indicators

In the prompt preview drawer, show which channels feed into the automation context:

- Display channel pills (Telegram, Discord, Teams, Email, Dashboard) with message counts
- Active channels (with messages in the context window) are highlighted; inactive channels are
  dimmed
- Each pill shows the count of messages from that channel in the current context window
- Channels are derived from the `messages.channel` column values in the fetched context

### Req-4: Filter Controls

Add lightweight filter controls in the drawer header:

- **Time range**: dropdown to select message lookback window (1h, 6h, 12h, 24h, 7d). Default: 24h.
  This controls the `messages` query time filter in the preview endpoint.
- **Obligation status**: multi-select chips for status filtering (open, in_progress, proposed_done).
  Default: open + in_progress.
- **Channel priority**: reorderable list or checkboxes to include/exclude specific channels from the
  context preview. Default: all enabled.

Filters update the preview in real-time (debounced 300ms). Filter state is local to the drawer
session -- not persisted.

### Req-5: Reminders vs Obligations Info Card

Add an info card component that appears in the "Scheduled Automations" section, between the section
header and the tab control. The card explains:

- **Obligations** = detected commitments from conversations. Have a lifecycle (open -> in_progress
  -> proposed_done -> done). Tracked with owner, priority, deadline, and source channel. Nova
  detects these automatically from messages.
- **Reminders** = one-shot scheduled alerts. Created explicitly ("remind me to..."). Can optionally
  be linked to an obligation (via `obligationId` FK). Delivered once at `dueAt` to a specific
  channel.

The card is collapsible (default collapsed) with an info icon trigger. Uses the existing
`ds-gray-alpha-100` surface style. Consider renaming the "Reminders" tab label to
"Reminders (Alerts)" and adding a subtitle to the "Obligations" section on other pages to reinforce
the distinction.

### Req-6: Loaded Context Summary Bar

Add a compact summary bar above each automation card showing key metrics from the loaded context:

- Active obligations count (with status breakdown: N open, M in progress)
- Memory topics loaded (count)
- Messages in context window (count, grouped by channel)
- Last context assembly timestamp

This bar provides at-a-glance visibility without opening the full drawer. It updates on the same
30-second refresh cycle as the main automations data.

## Scope

**IN**: Prompt preview drawer component, context preview tRPC endpoint, channel source pills,
filter controls (time range, obligation status, channel), reminders-vs-obligations info card,
context summary bar, TypeScript types for preview response.

**OUT**: Editing the static system prompt preamble (read-only in drawer), persisting filter
preferences across sessions, real-time WebSocket context updates, modifying the actual daemon
`gatherContext()` logic, adding new channel adapters, prompt version history or diff view.

## Impact

| Area | Change |
|------|--------|
| `packages/api/src/routers/automation.ts` | Add `previewContext` procedure with obligation/memory/message queries and channel aggregation |
| `apps/dashboard/app/automations/page.tsx` | Add PromptPreviewDrawer component, context summary bar, info card; wire "Preview Prompt" buttons to WatcherCard and BriefingCard |
| `apps/dashboard/types/api.ts` | Add `AutomationContextPreview`, `ChannelSummary`, `ContextStats` types |
| `apps/dashboard/app/globals.css` | Add drawer slide-in animation keyframes if not already present |

## Risks

| Risk | Mitigation |
|------|-----------|
| Context preview query performance -- querying obligations, memory, messages, and stats in parallel could be slow | Use `Promise.allSettled` with 5s timeout per query (same pattern as daemon `gatherContext`). Return partial results with source status indicators for unavailable sections. |
| Drawer component size adds to page bundle | Lazy-load the drawer via `React.lazy()` since it is only opened on demand. The drawer is not needed on initial page load. |
| Filter controls add complexity to the preview endpoint | Filters are applied client-side to the returned context data. The endpoint returns the full context; the drawer filters/sorts locally. This avoids multiple round-trips and keeps the API simple. |
| Channel list may be sparse (only Telegram active currently) | Show all known channels (Telegram, Discord, Teams, Email, Dashboard) with dimmed state for inactive ones. This communicates future capability and makes it clear which channels are feeding data. |
| Info card may be ignored if collapsed by default | Use a subtle pulsing dot on first visit (localStorage flag) to draw attention. After first expansion, the dot disappears permanently. |
