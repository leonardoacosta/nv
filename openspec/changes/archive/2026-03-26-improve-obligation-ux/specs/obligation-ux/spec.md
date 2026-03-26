# Capability: Obligation UX Improvements

## MODIFIED Requirements

### Requirement: Contextual action buttons per obligation status
`ObligationItem` SHALL render action buttons conditionally based on obligation status. Items with status `open` or `in_progress` SHALL show "Done" (`Check` icon) and "Cancel" (`X` icon) buttons. Items with status `proposed_done` SHALL show "Confirm Done" (`Check` icon) and "Reopen" (`RotateCcw` icon) buttons. Items with status `done` SHALL show only "Reopen" (`RotateCcw` icon). Items with status `dismissed` SHALL show no action buttons. All action buttons MUST use Lucide icon components with tooltip wrappers. The component MUST NOT render all three action types simultaneously.

#### Scenario: Active obligation shows Done and Cancel

Given an obligation with status `open`
When the obligation item renders
Then only the "Done" and "Cancel" icon buttons are visible
And hovering each button shows a tooltip with the action label

#### Scenario: Proposed done obligation shows Confirm and Reopen

Given an obligation with status `proposed_done`
When the obligation item renders
Then only the "Confirm Done" and "Reopen" icon buttons are visible
And the Cancel button is not rendered

#### Scenario: Completed obligation shows Reopen only

Given an obligation with status `done`
When the obligation item renders
Then only the "Reopen" icon button is visible

#### Scenario: Dismissed obligation shows no actions

Given an obligation with status `dismissed`
When the obligation item renders
Then no action buttons are rendered

### Requirement: Deadline proximity visual indicator
`ObligationItem` SHALL display an amber-to-red gradient border when the obligation deadline is within `approaching_deadline_hours` of the current time. Overdue obligations (deadline in the past) MUST display a solid red ring indicator. Obligations with no deadline or deadline beyond the threshold MUST NOT display any deadline indicator. The `approaching_deadline_hours` value SHALL be read from the API response; if absent, the component MUST fall back to 24 hours.

#### Scenario: Obligation approaching deadline shows amber indicator

Given an obligation with deadline 6 hours from now
And `approaching_deadline_hours` is 24
When the obligation item renders
Then the item displays an amber border/glow indicator

#### Scenario: Overdue obligation shows red indicator

Given an obligation with deadline 2 hours in the past
When the obligation item renders
Then the item displays a solid red ring indicator

#### Scenario: Distant deadline shows no indicator

Given an obligation with deadline 7 days from now
And `approaching_deadline_hours` is 24
When the obligation item renders
Then no deadline indicator is displayed

#### Scenario: No deadline shows no indicator

Given an obligation with no deadline set
When the obligation item renders
Then no deadline indicator is displayed

### Requirement: Inline expand/collapse for obligation details
`ObligationItem` SHALL hide detail content by default behind a collapsible container. Clicking the obligation row SHALL toggle the detail visibility with a CSS height-reveal animation of 150-200ms ease. The first item in the list MUST be expanded by default on mount. Expand/collapse state SHALL be tracked in parent component state via a `Set<string>` of expanded obligation IDs. State MUST persist within the browser session but is NOT required to persist across page reloads.

#### Scenario: Details collapsed by default

Given a list of 5 obligations
When the page renders
Then obligation details are hidden for items 2-5
And the first obligation's details are visible

#### Scenario: Click to expand

Given an obligation with collapsed details
When the user clicks the obligation row
Then the details expand with a smooth animation (150-200ms)

#### Scenario: Click to collapse

Given an obligation with expanded details
When the user clicks the obligation row header
Then the details collapse with a smooth animation (150-200ms)

### Requirement: Compact summary bar replaces stat cards
The obligations page SHALL replace the 5 individual stat cards with a single `ObligationSummaryBar` component. The summary bar MUST display status counts as colored inline badges in a horizontal layout (e.g., "3 Open | 2 In Progress | 1 Proposed Done | 5 Done | 0 Dismissed"). The summary bar MUST reduce vertical space from approximately 120px to approximately 40px.

#### Scenario: Summary bar shows correct counts

Given 3 open, 2 in-progress, 1 proposed-done, 5 done, and 0 dismissed obligations
When the obligations page renders
Then the summary bar displays "3 Open", "2 In Progress", "1 Proposed Done", "5 Done", "0 Dismissed" as colored badges

#### Scenario: Summary bar replaces stat cards

Given the obligations page loads
When the page renders
Then no individual stat cards are rendered
And a single compact summary bar is visible at the top
