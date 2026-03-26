# Proposal: Fix Briefing Page Crash

## Change ID
`fix-briefing-crash`

## Summary
Fix the /briefing page client-side exception that crashes the entire page, add an error boundary so controls remain usable, and harden data access against null API responses.

## Context
- Extends: `apps/dashboard/app/briefing/page.tsx`, `apps/dashboard/lib/briefing.ts`, `apps/dashboard/types/api.ts`
- Related: `add-morning-briefing` (active spec — this is a hotfix for the existing implementation)

## Motivation
The /briefing page crashes with a full-viewport "Application error" when the daemon returns unexpected data shapes. The page is completely unusable — no error recovery, no history access, no refresh button. This is P0: the briefing is a primary daily touchpoint in the Daily Check-in journey (Dashboard -> Obligations -> Approvals -> Briefing).

## Requirements

### Req-1: Null-safe data access
All property accesses on `BriefingEntry` fields (`content`, `sources_status`, `suggested_actions`) must be guarded against null/undefined. The `BriefingGetResponse.entry` field must handle null (daemon may return `{ entry: null }` when no briefing exists).

### Req-2: React error boundary
Wrap the briefing content renderer (section cards, sources, suggested actions) in a React error boundary. When the content area crashes, the page header (title, refresh button) and history rail must remain functional so the user can retry or navigate to a different briefing.

### Req-3: Graceful degradation for partial data
If `sources_status` is missing, skip rendering the sources bar. If `suggested_actions` is missing or empty, skip the actions section. If `content` is null/empty, show the empty state. Never crash on missing optional fields.

## Scope
- **IN**: Null guards on briefing data, error boundary component, defensive rendering for partial data
- **OUT**: New briefing features, typewriter animations (separate spec), briefing generation logic (daemon-side)

## Impact
| Area | Change |
|------|--------|
| `app/briefing/page.tsx` | Add null guards, wrap content in error boundary |
| `components/layout/ErrorBoundary.tsx` | New reusable error boundary component |
| `types/api.ts` | Make `BriefingGetResponse.entry` nullable |

## Risks
| Risk | Mitigation |
|------|-----------|
| Error boundary hides real bugs | Include "Show details" toggle in error boundary fallback UI |
| Type change to nullable entry breaks other consumers | Grep for all `BriefingGetResponse` usage — only briefing page uses it |
