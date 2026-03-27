# Proposal: Automations UX Overhaul

## Change ID
`automations-ux-overhaul`

## Summary

Redesign the `/automations` page from a five-section vertical stack of mostly-empty tables into a
compact grid layout with inline documentation, watcher editing controls, improved briefing controls,
and a cross-link to the Sessions page. The current page shows 5 stacked sections where only 2
(Watcher, Briefing) have data -- the rest display "0" with no guidance on how to populate them and
no creation affordances.

## Context
- Phase: Dashboard UX improvement (post-`add-automations-page` spec)
- Extends: `apps/dashboard/app/automations/page.tsx` (current 544-line page with 5 vertical sections)
- Related specs: `add-automations-page` (original implementation, now in changes/)
- Key sources:
  - `apps/dashboard/app/automations/page.tsx` -- current page component
  - `apps/dashboard/app/api/automations/route.ts` -- GET endpoint aggregating reminders, schedules, sessions, briefings, watcher config
  - `apps/dashboard/app/api/automations/reminders/[id]/route.ts` -- PATCH cancel
  - `apps/dashboard/app/api/automations/schedules/[id]/route.ts` -- PATCH enable/disable
  - `apps/dashboard/app/api/briefing/generate/` -- POST generate briefing (already exists)
  - `apps/dashboard/app/sessions/page.tsx` -- dedicated Sessions page
  - `apps/dashboard/types/api.ts` -- `AutomationsGetResponse` and related types
  - `apps/dashboard/components/layout/SectionHeader.tsx` -- shared section header component
  - `apps/dashboard/components/layout/PageShell.tsx` -- shared page layout shell

## Motivation

The automations page was built as a data-display page during Wave 1 but currently fails as a
management interface:

1. **Wasted space** -- 5 vertically-stacked sections where 3 consistently show "0" or "No active X"
   create a long, empty-feeling page. The useful content (Watcher status, Briefing schedule) is
   buried below the empty sections.

2. **No creation guidance** -- there are no "Create Reminder" or "Create Schedule" buttons, and no
   explanation of how to create them (Telegram commands, daemon config, API). A new user has no idea
   how to populate these sections.

3. **No inline editing** -- the Watcher shows status/interval/quiet-hours as read-only text. The
   only way to change these settings is to edit daemon config and restart. Similarly, the Briefing
   section does not surface the existing "Generate Now" button from the Briefing page.

4. **Active Sessions duplication** -- the Active Sessions section duplicates the dedicated
   `/sessions` page in a less useful way (no search, no detail panel, no filters). When empty, it
   adds noise; when populated, it is inferior to the real Sessions page.

5. **Missing context** -- each automation type serves a different purpose with different management
   patterns, but nothing on the page explains what they do or how to interact with them.

## Requirements

### Req-1: Grid Layout Restructure

Replace the 5-section vertical stack with a 2-column grid layout:

- **Top row**: Two stat cards side by side:
  - **Watcher card** (left) -- shows enabled status, interval, quiet hours, last run. These are the
    fields that always have data.
  - **Briefing card** (right) -- shows last generated time, next generation time, schedule. These
    always have data.
- **Bottom row**: Single full-width section combining Reminders and Schedules into a unified
  "Scheduled Automations" table/list with tabs or a segmented control to switch between the two
  types.
- Remove the Active Sessions section entirely (see Req-6 for cross-link).
- On mobile (< 768px), the top row cards stack vertically.

### Req-2: Inline Documentation

Add a one-line description to each section explaining what it does and how items are managed. These
appear as muted helper text below each section header:

- **Reminders tab**: "Created via Telegram ('remind me to...') or the API. One-time alerts delivered
  to a channel."
- **Schedules tab**: "Recurring jobs configured in daemon schedule-svc config. Toggle enabled/disabled
  here."
- **Watcher card**: "Monitors Telegram channels for actionable items. Configured via daemon watcher
  settings."
- **Briefing card**: "Generates a daily summary at the configured time. Trigger manually with
  Generate Now."

Each description is a `<p>` with `text-copy-13 text-ds-gray-700` styling, placed directly below the
`SectionHeader` component.

### Req-3: Watcher Inline Editing

Convert the Watcher card from read-only display to an editable control surface:

- **Active toggle**: A toggle switch to enable/disable the watcher. Calls a new
  `PATCH /api/automations/watcher` endpoint with `{ "enabled": true|false }`.
- **Interval adjustment**: A number input or stepper to adjust `interval_minutes` (min 5, max 120,
  step 5). Submits on blur or Enter via the same PATCH endpoint with
  `{ "interval_minutes": number }`.
- **Quiet hours**: Two time inputs for `quiet_start` and `quiet_end` (24h format select or time
  picker). Submits via the same PATCH endpoint with `{ "quiet_start": "HH:MM", "quiet_end": "HH:MM" }`.

All edits are optimistic: update local state immediately, revert on API failure with an inline error
toast.

**New API endpoint**: `PATCH /api/automations/watcher` -- accepts partial update body with any
combination of `enabled`, `interval_minutes`, `quiet_start`, `quiet_end`. Updates the corresponding
environment variables or config source. Returns the updated watcher state.

### Req-4: Briefing Card Controls

Enhance the Briefing card to surface actions already available elsewhere:

- **Generate Now button**: A button that calls `POST /api/briefing/generate` (endpoint already
  exists at `apps/dashboard/app/api/briefing/generate/`). Shows a loading spinner while generating.
  On success, refreshes the briefing data to show the updated `last_generated_at`.
- **Last briefing preview**: Show a truncated (2-3 line) preview of the most recent briefing
  content. Clicking it navigates to `/briefing` for the full view.
- **View full briefing link**: A "View Briefing" text link navigating to `/briefing`.

**New API field**: The `AutomationBriefing` type needs an additional optional field
`content_preview: string | null` -- a truncated snippet (first 200 chars) of the latest briefing
content. The `GET /api/automations` endpoint adds this by selecting `content` from the latest
briefing row and truncating.

### Req-5: Future Create UI Placeholder

Do NOT implement a full "New Reminder" creation dialog in this spec. Instead:

- In the Reminders tab empty state, show actionable guidance: "No active reminders. Tell Nova
  'remind me to...' in Telegram, or create one via the API." with a subtle info icon.
- In the Schedules tab empty state: "No scheduled jobs. Schedules are configured in the daemon
  schedule-svc config."
- These empty states replace the current plain "No active reminders" / "No scheduled jobs" text.

### Req-6: Sessions Cross-Link

Remove the Active Sessions section from the automations page. Replace it with a single-line link in
the page header area or below the grid:

- Text: "N active sessions" (when sessions exist) or "No active sessions" (when empty), rendered as
  a clickable link to `/sessions`.
- Uses the existing `active_sessions.length` from the API response for the count.
- Styled as a subtle inline link (`text-copy-13 text-ds-gray-700 hover:text-ds-gray-1000
  underline-offset-2 hover:underline`).

### Req-7: Type Updates

Extend `AutomationBriefing` in `apps/dashboard/types/api.ts`:

```typescript
export interface AutomationBriefing {
  last_generated_at: string | null;
  next_generation: string | null;
  content_preview: string | null; // NEW: first 200 chars of latest briefing
}
```

No other type changes needed -- the watcher PATCH endpoint returns `AutomationWatcher` which already
exists.

## Scope

**IN**: Grid layout restructure, inline documentation text, watcher inline editing (toggle +
interval + quiet hours), briefing generate button + content preview, sessions cross-link, empty state
improvements, `PATCH /api/automations/watcher` endpoint, `content_preview` field on briefing
response.

**OUT**: Full reminder creation dialog (deferred -- requires Telegram bot integration design),
schedule creation/editing UI (schedules are daemon-config-managed), real-time WebSocket updates,
watcher log/history view, briefing scheduling configuration, changes to the Sessions page itself.

## Impact

| Area | Change |
|------|--------|
| `apps/dashboard/app/automations/page.tsx` | Major rewrite: grid layout, watcher editing, briefing controls, inline docs, sessions cross-link |
| `apps/dashboard/app/api/automations/route.ts` | Modified: add `content_preview` to briefing response by selecting latest briefing content |
| `apps/dashboard/app/api/automations/watcher/route.ts` | New: PATCH handler for watcher config updates (enabled, interval, quiet hours) |
| `apps/dashboard/types/api.ts` | Modified: add `content_preview` to `AutomationBriefing` |

## Risks

| Risk | Mitigation |
|------|-----------|
| Watcher config is currently read from env vars (`process.env`), not a DB table -- PATCH cannot persist changes across restarts | For v1, the PATCH endpoint updates in-memory state and writes to a `.env.local` or config override file. Document that daemon restart resets to base config. A future spec can migrate watcher config to a DB settings table. |
| Briefing content may be large (markdown with multiple sections) | The `content_preview` field truncates to 200 chars server-side and strips markdown formatting. The full content is only loaded on the `/briefing` page. |
| Grid layout may feel sparse if both Watcher and Briefing cards are small | Cards use `min-h-[120px]` and stretch to fill available width in a `grid-cols-2` layout. If content is minimal, the compact layout is still an improvement over 5 stacked empty sections. |
| Optimistic updates for watcher editing could desync with actual config state | On any PATCH failure, revert local state and show an inline error message. The 30s auto-refresh acts as a safety net to resync. |
| Removing Active Sessions section could be a regression for users who relied on it | The sessions count + link in the header provides the same at-a-glance info. The dedicated `/sessions` page is strictly superior for detail. |
