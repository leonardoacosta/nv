# Spec: Action Items Panel

## ADDED Requirements

### Requirement: Surface actionable items at page top

The dashboard home page SHALL display an "Action Items" section immediately below the page header that aggregates all items requiring user attention from three sources: pending obligations, unread messages, and failed automations.

Each action item MUST render as a single dense row with severity dot, category label, one-line description, and navigation link. The section SHALL collapse to "All clear" when empty and MUST cap visible items at 10 with an expand toggle.

#### Scenario: Pending obligations appear as action items

Given the obligation list includes items with status "open" or "in_progress"
When the dashboard home page renders
Then each pending obligation appears as a warning-severity action item with the obligation's detected_action as summary and a link to /obligations.

#### Scenario: Unread messages appear as action items

Given inbound messages exist from the last 4 hours where sender is not "nova"
When the dashboard home page renders
Then a single action item summarizes unread messages (e.g., "5 unread messages -- 3 from Telegram, 2 from Discord") with a link to /messages.

#### Scenario: Failed automations appear as action items

Given overdue reminders or stopped sessions exist in the automation overview
When the dashboard home page renders
Then each failed automation appears as an error-severity action item with a link to /automations.

#### Scenario: No action items exist

Given no pending obligations, unread messages, or failed automations exist
When the dashboard home page renders
Then the Action Items section shows a single "All clear" line in muted text with no rows.

#### Scenario: More than 10 action items

Given 15 actionable items exist across all sources
When the dashboard home page renders
Then 10 items are visible with a "5 more" toggle that expands to show the remaining items.

## REMOVED Requirements

### Requirement: Priority Banner

The PriorityBanner component (amber banner for obligations, blue banner for briefing) is removed from the home page. Its signals are absorbed into the Action Items panel (obligations) and Nova's Status section (briefing availability).
