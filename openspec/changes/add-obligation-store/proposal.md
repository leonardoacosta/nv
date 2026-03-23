# Proposal: Add Obligation Store

## Change ID
`add-obligation-store`

## Summary
Create obligations SQLite table, Rust types, and CRUD operations for cross-channel obligation tracking.

## Context
- Extends: messages.rs (migration), nv-core/types.rs
- Related: PRD FR-3, FR-4

## Motivation
Obligation detection needs a persistence layer. Obligations must survive daemon restarts and be queryable by status, owner, project, and priority.

## Requirements

### Req-1: Obligations table
Create obligations table via rusqlite_migration with columns: id, source_channel, source_message, detected_action, project_code, priority, status, owner, owner_reason, timestamps.

### Req-2: Rust types and CRUD
ObligationStore with create, list_by_status, list_by_owner, update_status, count_open methods.

## Scope
- **IN**: Schema migration, Rust types, CRUD operations, unit tests
- **OUT**: Detection logic, API endpoints, UI

## Impact
| Area | Change |
|------|--------|
| messages.rs | Migration v2: obligations table |
| new obligation_store.rs | ObligationStore struct + methods |
| nv-core/types.rs | Obligation, ObligationStatus, ObligationOwner enums |

## Risks
| Risk | Mitigation |
|------|-----------|
| Schema changes later | rusqlite_migration handles incremental changes |
