# obligation-clarity Specification

## Purpose
TBD - created by archiving change add-automations-prompt-preview. Update Purpose after archive.
## Requirements
### Requirement: Reminders vs obligations info card

The dashboard SHALL display an info card in the "Scheduled Automations" section that explains the
distinction between obligations (detected commitments with lifecycle) and reminders (one-shot alerts
optionally linked to obligations). The card MUST be collapsible, collapsed by default, with a
first-visit attention indicator.

#### Scenario: First-time user sees attention indicator

Given the user has never expanded the info card (no localStorage flag set)
When the automations page loads
Then a subtle pulsing dot appears on the info card trigger icon, and after the user expands the
card once, the dot disappears permanently (localStorage flag set).

#### Scenario: Info card explains the distinction

Given the user expands the info card
When the card content renders
Then it shows: "Obligations" described as detected commitments with lifecycle (open -> in_progress
-> proposed_done -> done), tracked with owner/priority/deadline/source channel, auto-detected
from messages; and "Reminders" described as one-shot scheduled alerts created explicitly, optionally
linked to an obligation via FK, delivered once at due time to a specific channel.

### Requirement: Tab label clarification

The "Reminders" tab in the segmented control SHALL be renamed to "Reminders (Alerts)" to
distinguish from obligations.

#### Scenario: Updated tab label renders correctly

Given the automations page loads with data
When the segmented control tabs render
Then the first tab reads "Reminders (Alerts)" with the Bell icon and count, and the second tab
reads "Schedules" unchanged.

