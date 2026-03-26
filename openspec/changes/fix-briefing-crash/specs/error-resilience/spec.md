# Spec: Error Resilience for Briefing Page

## MODIFIED Requirements

### Requirement: Null-safe briefing data access

The briefing page MUST guard all property accesses on `BriefingEntry` fields (`content`, `sources_status`, `suggested_actions`) against null/undefined values. `BriefingGetResponse.entry` SHALL be typed as nullable to match the daemon's actual response shape when no briefing exists.

#### Scenario: API returns entry with null sources_status

Given the daemon returns a BriefingEntry where `sources_status` is null or undefined,
when the briefing page renders,
then the sources status bar is not rendered and no error is thrown.

#### Scenario: API returns entry with null content

Given the daemon returns a BriefingEntry where `content` is null or undefined,
when the briefing page renders,
then the empty state is shown ("No briefing yet today") and no error is thrown.

#### Scenario: API returns null entry

Given the daemon returns `{ entry: null }` for GET /api/briefing,
when the briefing page renders,
then the empty state is shown and the history rail remains functional.

## ADDED Requirements

### Requirement: Reusable ErrorBoundary component

The dashboard MUST provide a reusable React ErrorBoundary class component at `components/layout/ErrorBoundary.tsx` that catches render errors in its children and displays a fallback UI with retry action.

#### Scenario: Child component throws during render

Given a component wrapped in ErrorBoundary throws an error,
when the error is caught,
then the fallback UI is rendered with the error message, a retry action is provided, and surrounding layout outside the boundary is unaffected.

#### Scenario: Error boundary reset on retry

Given the error boundary is in error state,
when the user clicks the retry action,
then the error boundary resets its state and attempts to re-render the children.

### Requirement: Briefing content isolation

The briefing page MUST wrap the content renderer (section cards, sources, suggested actions) in the ErrorBoundary. The page header (title, refresh button) and history rail SHALL remain outside the boundary so they stay functional when the content area crashes.

#### Scenario: Content renderer throws unexpected error

Given an unhandled error occurs in the briefing content area,
when the error boundary catches the exception,
then the page header and history rail remain visible and functional, an error message with "Try refreshing" action is shown in the content area, and error details are available via a "Show details" toggle.
