# Spec: Nova's Status Section

## ADDED Requirements

### Requirement: Nova operational status display

A "Nova's Status" section SHALL render between Action Items and Activity Summaries showing three data points in a single horizontal row: connected channels, active watchers, and last briefing time. This MUST consolidate information previously spread across StatStrip cells and CcSessionsWidget.

#### Scenario: Connected channels displayed

Given the fleet status response includes channels (Telegram: configured, Discord: configured, Microsoft Teams: configured)
When Nova's Status renders
Then each channel name appears with a green dot for "configured" status or a red dot for other statuses, displayed inline in the first cell.

#### Scenario: Watcher state displayed

Given the watcher is enabled with a 30-minute interval
When Nova's Status renders
Then the second cell shows "Enabled" with "every 30m" in muted text below.

#### Scenario: Watcher disabled

Given the watcher is disabled
When Nova's Status renders
Then the second cell shows "Disabled" in muted text.

#### Scenario: Last briefing time displayed

Given a briefing was generated 2 hours ago
When Nova's Status renders
Then the third cell shows "2h ago" in monospace font.

#### Scenario: No briefing exists

Given no briefing has been generated
When Nova's Status renders
Then the third cell shows "None" in muted text.

## REMOVED Requirements

### Requirement: StatStrip on home page

The StatStrip component (Unread Messages, Pending Obligations, Fleet Health, Active Sessions, Next Briefing cells) is removed from the home page. Its signals are redistributed: unread messages and pending obligations move to Action Items, fleet/channel status moves to Nova's Status, session count is visible in the Sessions activity group, briefing status moves to Nova's Status.

### Requirement: CC Sessions Widget on home page

The CcSessionsWidget card linking to /sessions is removed. Active session count is visible in the Sessions activity summary group header.

## ADDED Requirements

### Requirement: Collapsible Quick Add

The ObligationBar SHALL move from its fixed position above the activity feed to a collapsible "Quick Add" row below the activity summaries. A "+" button MUST expand the inline input; successful creation SHALL auto-collapse with a brief confirmation.

#### Scenario: Quick Add collapsed by default

Given the dashboard home page loads
When the page renders
Then the Quick Add row shows only a "+" icon button, no input field visible.

#### Scenario: Expanding Quick Add

Given Quick Add is collapsed
When the user clicks the "+" button
Then the obligation input field and submit button expand inline.

#### Scenario: Auto-collapse after creation

Given Quick Add is expanded and the user submits a new obligation
When the creation succeeds
Then the input collapses back to the "+" button with a brief "Created" confirmation that fades after 2 seconds.
