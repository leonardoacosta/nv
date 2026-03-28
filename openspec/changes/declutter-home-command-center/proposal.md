# Proposal: Declutter Home Command Center

## Change ID
`declutter-home-command-center`

## Summary

Replace the flat activity feed dump with categorized activity summaries, surface actionable items at the top, add a "Nova's Status" card, and reduce visual noise through better grouping and progressive disclosure.

## Context
- Extends: `apps/dashboard/app/page.tsx` (881-line single-file command center), `packages/api/src/routers/system.ts` (activityFeed procedure), `apps/dashboard/types/api.ts`
- Related: `redesign-dashboard-home` (70% complete, built the current page structure including PriorityBanner, StatStrip, ActivityFeed, ObligationBar, RecentConversations), `global-density-pass` (established Geist design tokens and density targets)

## Motivation

The `redesign-dashboard-home` spec successfully replaced stat cards with a dense command center layout, but the activity feed remains a flat chronological dump of all events. With 50 events across 4 types (messages, obligations, diary, sessions), the feed produces visual noise that buries actionable information. The existing category pills (All/Messages/Sessions/Obligations/System) help filter but do not summarize or prioritize.

Specific problems:

1. **No hierarchy** -- a failed obligation and a routine diary entry have equal visual weight unless severity coloring happens to apply; scanning 50 rows to find what needs attention is slow
2. **No summarization** -- 12 inbound Telegram messages appear as 12 separate rows instead of "12 messages from Telegram (3 unread)"; the volume is the noise
3. **Actionable items buried** -- pending obligations, unread messages, and failed automations are mixed into the chronological stream; the PriorityBanner only surfaces obligation count and briefing availability, not specific action items
4. **Missing Nova operational status** -- no single place shows connected channels, active watchers, and last briefing time; the StatStrip has fragments (fleet health, next briefing) but not the full picture
5. **Visual clutter** -- the page stacks PriorityBanner + StatStrip + CcSessionsWidget + CategoryPills + ObligationBar + ActivityFeed + RecentConversations vertically; too many distinct sections compete for attention

## Requirements

### Req-1: Action Items Panel

Replace the PriorityBanner with an "Action Items" section at the top of the page that aggregates all items requiring user attention. Query pending obligations (status "open" or "in_progress"), unread messages (inbound messages in last 4 hours where sender is not "nova"), and failed automations (from `automation.getAll` -- overdue reminders, stopped sessions). Each action item renders as a single row: severity dot (red/amber), category label, one-line description, and a link to the relevant page. When no action items exist, the section collapses to a single "All clear" line in muted text. Maximum 10 items shown; if more exist, show a "N more" link expanding to the full list.

### Req-2: Categorized Activity Summaries

Replace the flat chronological activity feed with a grouped view. Group activity feed events by type (messages, obligations, sessions, system) and within each group show a summary header line: icon, category name, event count, and a one-line summary (e.g., "8 messages -- 3 inbound from Telegram, 5 outbound"). Below each summary header, show the 3 most recent events in that category as compact rows (same dense format as current feed). Each group has a "View all" link that navigates to the relevant page (/messages, /obligations, /sessions, /diary). Groups with zero events are hidden. Groups are ordered by recency of their most recent event.

### Req-3: Nova's Status Card

Add a "Nova's Status" section (rendered as a bordered container, not a card with shadow) showing three data points in a single horizontal row: (1) connected channels -- list channel names from `system.fleetStatus` channels array with a green/red dot per channel based on status, (2) active watchers -- enabled/disabled state and interval from `automation.getAll` watcher data, (3) last briefing -- timestamp from `briefing.latest` formatted as relative time ("2h ago") or "None" if no briefing exists. Place this below the Action Items and above the activity summaries.

### Req-4: Progressive Disclosure for Activity Detail

Each category summary group is collapsed by default (showing only the summary header + 3 recent items). Clicking the summary header or a "Show more" control expands the group to show all events in that category (up to 50 from the API). Only one group can be expanded at a time -- expanding a group collapses any previously expanded group. The expanded view uses the same dense row format with severity coloring and expandable detail panels from the current implementation.

### Req-5: Reduce Visual Noise

Remove the CategoryPills filter tabs (replaced by grouped view in Req-2). Move the ObligationBar into a collapsible "Quick Add" row that shows a "+" button; clicking it expands the inline input. Consolidate the StatStrip and CcSessionsWidget into the Nova's Status section (Req-3) -- remove the separate StatStrip component usage and CcSessionsWidget from the page. Keep the RecentConversations section but reduce it to 3 groups (from 5) and move it into the activity summaries as the "Messages" group detail. Remove the standalone RecentConversations section header.

### Req-6: Aggregated Real-Time Updates

Keep the existing `useDaemonEvents` WebSocket subscription but instead of prepending raw WebSocket events to the feed, increment category counters. Show a subtle "N new events" badge on the relevant category summary header. Clicking the badge triggers a refresh of that category's data from the API. Do not auto-refresh the entire feed on each WebSocket event -- only update the badge counters. Keep the existing auto-refresh toggle for full periodic refreshes.

### Req-7: Layout and Styling

Maintain the Geist/Vercel dark theme design language: `ds-*` tokens, monospace for data, subtle borders, tight spacing. The page layout is single-column (remove the unused 60/40 grid -- the current page already uses single-column). Section order top-to-bottom: page header with refresh controls, Action Items, Nova's Status, Activity Summaries (grouped), collapsed Quick Add. Use `border-b border-ds-gray-400` between sections, not gaps or cards. The page header keeps "Command Center" title with auto-refresh toggle and "Updated Xs ago" timestamp.

## Scope
- **IN**: Dashboard home page refactor (page.tsx), new API procedure for action items aggregation (or client-side derivation from existing queries), Nova's Status section, categorized activity summaries with progressive disclosure, WebSocket badge counters, ObligationBar collapse behavior, removal of CategoryPills/StatStrip/CcSessionsWidget from home page
- **OUT**: Activity feed API changes (existing `system.activityFeed` procedure is sufficient), other dashboard pages, daemon WebSocket protocol changes, new database tables, StatStrip component deletion (other pages may use it), automation router changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/page.tsx` | Major refactor: replace PriorityBanner + StatStrip + CcSessionsWidget + CategoryPills + flat ActivityFeed + RecentConversations with ActionItems + NovaStatus + GroupedActivitySummaries + collapsible QuickAdd |
| `apps/dashboard/types/api.ts` | Add: `ActionItem`, `CategorySummary` types (client-side derived, no API change needed) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Removing StatStrip/CcSessionsWidget from home may lose at-a-glance metrics users rely on | Nova's Status section absorbs the essential signals (channels, watchers, briefing); fleet health dot moves there; session count moves to Action Items if sessions are running |
| Grouped view hides events that cross category boundaries | "All" is not a group -- the summary headers with counts give a complete picture; expanding any group shows full detail |
| WebSocket badge counter may drift from actual event count | Badge is approximate ("new events" not exact count); clicking it refreshes from API which is the source of truth |
| Single-column layout with multiple sections may still require scrolling | Progressive disclosure (collapsed groups, collapsed QuickAdd) keeps above-the-fold content to ActionItems + NovaStatus + 4 summary headers -- roughly 400px |
