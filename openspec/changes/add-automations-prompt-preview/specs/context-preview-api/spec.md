# Context Preview API

## ADDED Requirements

### Requirement: automation.previewContext procedure

The automation router SHALL expose a `previewContext` procedure that returns the assembled context
for a given automation type without triggering the automation run. It MUST query obligations,
memory, messages, and stats in parallel with timeouts, returning structured sections with source
status indicators.

#### Scenario: Briefing context preview returns all sections

Given the user calls `automation.previewContext` with `{ type: "briefing" }`
When obligations, memory, and messages tables contain data
Then the response includes `obligations.items` (up to 20 active), `memory.items` (up to 10 topics
with content preview), `messages.byChannel` (grouped with counts and latest preview),
`channels` (list with name, messageCount, active flag), `stats` (totalObligations,
activeReminders, memoryTopics), and `assembledAt` ISO timestamp.

#### Scenario: Watcher context preview returns obligation-focused context

Given the user calls `automation.previewContext` with `{ type: "watcher" }`
When obligations exist with various statuses
Then the response includes `obligations.countByStatus` with counts for open, in_progress, and
proposed_done, and `obligations.items` limited to 20 ordered by priority ASC then created_at ASC.

#### Scenario: Partial data availability returns source status

Given one or more context sources (obligations, memory, messages) are unavailable or empty
When the preview procedure runs with `Promise.allSettled`
Then each section includes a `sourceStatus` field of "ok", "empty", or "unavailable", and
available sections return normally while unavailable sections return empty arrays with the
"unavailable" status.
