# Capability: System Pages Polish

## MODIFIED Requirements

### Requirement: Settings page groups items into collapsible sections
The Settings page SHALL organize all settings into four collapsible categories: General, Network, Scheduling, and Advanced. Each section header MUST display the category name and item count. Sections MUST default to expanded on first load. Expanded/collapsed state MUST persist in localStorage across page navigations.

#### Scenario: User collapses a section
Given the Settings page is loaded with all sections expanded
When the user clicks the "Network" section header
Then the Network section collapses with a CSS transition
And the collapsed state is saved to localStorage
And on next page load the Network section renders collapsed

#### Scenario: Save confirmation flash
Given a user edits a settings field and saves
When the save completes successfully
Then the saved field row shows a green background flash lasting 300ms
And the flash fades out via CSS transition

### Requirement: Dirty state bar for restart-required fields
When one or more restart-required settings fields are modified, the page SHALL show a fixed-position floating bar at the viewport bottom. The bar MUST display the count of unsaved changes and offer "Save & Restart" and "Discard" actions. The bar MUST disappear after save or discard.

#### Scenario: First dirty field shows bar
Given no restart-required fields are modified
When the user changes a restart-required field value
Then the floating "Save & Restart" bar appears at the bottom with "1 unsaved change"

#### Scenario: Multiple dirty fields update count
Given the floating bar shows "1 unsaved change"
When the user modifies a second restart-required field
Then the bar updates to "2 unsaved changes"

#### Scenario: Discard clears dirty state
Given the floating bar shows "2 unsaved changes"
When the user clicks "Discard"
Then all restart-required fields revert to their saved values
And the floating bar disappears

## ADDED Requirements

### Requirement: Integration avatars use deterministic hash-based colors
Each integration card avatar background color SHALL be derived deterministically from the service name using a hash function mapped to an 8-color curated palette. The same service name MUST always produce the same color. Disconnected integrations MUST render at opacity 0.6. Connected integrations MUST render at full opacity with a subtle elevation shadow.

#### Scenario: Same service always gets same color
Given integrations "GitHub" and "Jira" exist
When the Integrations page renders
Then "GitHub" avatar background is always the same palette color across page loads
And "Jira" avatar background is always the same palette color across page loads
And the two colors may or may not differ (determined by hash)

#### Scenario: Disconnected integration is dimmed
Given integration "Slack" has status "Disconnected"
When the IntegrationCard renders
Then the card renders at opacity 0.6
And no elevation shadow is applied

#### Scenario: Connected integration is elevated
Given integration "GitHub" has status "Connected"
When the IntegrationCard renders
Then the card renders at full opacity
And a subtle box-shadow elevation is applied

### Requirement: Connected status badges pulse green
"Connected" status badges SHALL display a green glow-pulse animation using a 2s ease-in-out infinite CSS animation. The animation MUST use box-shadow only and MUST NOT cause layout shift.

#### Scenario: Pulse visible on connected badge
Given integration "GitHub" has status "Connected"
When the status badge renders
Then a green glow-pulse box-shadow animation plays continuously at 2s intervals

#### Scenario: No pulse on disconnected badge
Given integration "Slack" has status "Disconnected"
When the status badge renders
Then no pulse animation is applied

### Requirement: Memory detail panel renders markdown
The Memory page detail panel SHALL render file content as formatted markdown. The file list sidebar SHALL display last-modified timestamp and word count for each file.

#### Scenario: Markdown headers and lists render
Given a memory file contains `# Title\n\n- item one\n- item two`
When the file is selected in the Memory page
Then the detail panel renders "Title" as an h1 heading
And renders a bulleted list with "item one" and "item two"

#### Scenario: File metadata shown in list
Given a memory file was last modified at 2026-03-20T14:30:00Z and contains 142 words
When the Memory file list renders
Then the file entry shows "Mar 20, 2026" (or equivalent formatted date)
And shows "142 words"

#### Scenario: Empty file shows placeholder
Given a memory file has empty content
When the file is selected
Then the detail panel shows a "(No content)" placeholder instead of blank space
