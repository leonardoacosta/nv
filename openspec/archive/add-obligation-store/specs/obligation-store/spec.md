# Capability: Obligation Store

## ADDED Requirements

### Requirement: Obligation persistence
The daemon MUST persist obligations in SQLite with full lifecycle tracking (open, acknowledged, handled, dismissed).

#### Scenario: Create obligation
**Given** an obligation is detected from a Discord message
**When** ObligationStore::create is called
**Then** the obligation is persisted with source_channel, message, project, priority, and status "open"

#### Scenario: Query by owner
**Given** 3 obligations exist (2 for leo, 1 for nova)
**When** list_by_owner("leo") is called
**Then** 2 obligations are returned
