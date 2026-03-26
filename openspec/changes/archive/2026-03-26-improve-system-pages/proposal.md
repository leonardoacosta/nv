# Proposal: Improve System Pages

## Change ID
`improve-system-pages`

## Summary

Polish Settings, Integrations, and Memory pages -- group settings into collapsible sections, add hashed avatar colors for integrations, and render memory files as formatted markdown.

## Context
- Extends: `apps/dashboard/app/settings/page.tsx`, `apps/dashboard/app/integrations/page.tsx`, `apps/dashboard/app/memory/page.tsx`
- Related: existing dashboard layout and component library

## Motivation

Settings has 28 items in a flat list (hard to navigate). Integration avatars are all identical dark gray (hard to scan). Memory files show raw text or "(empty)" with no formatting. These are low-traffic but important pages that feel unfinished.

1. **Settings discoverability** -- 28 ungrouped items force users to scroll and scan linearly. Collapsible sections with item counts let users jump to the right category.
2. **Integration visual identity** -- identical dark gray avatars make it impossible to distinguish services at a glance. Deterministic hash-based colors create instant visual anchors.
3. **Memory readability** -- raw unformatted text defeats the purpose of a detail panel. Markdown rendering makes memory files useful without leaving the dashboard.

## Requirements

### Req-1: Settings section grouping

Organize 28 settings into collapsible categories (General, Network, Scheduling, Advanced) with item counts. Each section header shows the category name and the number of items it contains. Sections default to expanded on first load. Save confirmation flash (green background 300ms) on field save.

### Req-2: Settings dirty state

When restart-required fields are modified, show a floating "Save & Restart" bar at the bottom of the viewport with an unsaved changes count badge. The bar appears on first dirty field and disappears after save or discard.

### Req-3: Integration avatar colors

Generate deterministic background colors from service name hash using an 8-color curated palette. Dim disconnected items (opacity 0.6), elevate connected ones (full opacity, subtle shadow). The same service name always produces the same color.

### Req-4: Connected pulse

Subtle green glow-pulse animation on "Connected" status badges using a 2s ease-in-out infinite CSS animation. The pulse should be unobtrusive -- no layout shift, purely a box-shadow or outline effect.

### Req-5: Memory markdown preview

Render memory file content as formatted markdown in the detail panel. Show file metadata (last-modified timestamp, word count) in the file list sidebar. Use a lightweight markdown renderer (marked, remark, or simple regex-based rendering for headers/bold/lists).

## Scope
- **IN**: Settings grouping and collapsible sections, settings dirty state bar, integration avatar hash colors, connected pulse animation, memory markdown rendering and file metadata
- **OUT**: Settings schema changes, new integration connectors, memory CRUD operations (create/edit/delete), new API endpoints, database migrations

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/settings/page.tsx` | Modified: refactor flat list into collapsible grouped sections with save flash |
| `apps/dashboard/app/settings/` (new components) | New: `SettingsSection.tsx`, `SaveRestartBar.tsx` |
| `apps/dashboard/app/integrations/page.tsx` | Modified: avatar color generation, connected/disconnected visual states |
| `apps/dashboard/app/integrations/` (components) | Modified: `IntegrationCard.tsx` -- hash-based avatar colors, opacity dimming |
| `apps/dashboard/app/memory/page.tsx` | Modified: markdown rendering in detail panel, file metadata in list |

## Risks
| Risk | Mitigation |
|------|-----------|
| Markdown rendering adds a dependency | Use a lightweight renderer (marked or remark) already common in the ecosystem, or simple regex-based rendering for headers/bold/lists only |
| Collapsible section state lost on navigation | Store expanded/collapsed state in localStorage keyed by section name |
| Hash color palette clashes with theme | Curate 8 colors that work on both light and dark backgrounds; test against existing dashboard theme |
| Save flash timing feels jarring | 300ms green background fade with CSS transition; user-testable threshold |
