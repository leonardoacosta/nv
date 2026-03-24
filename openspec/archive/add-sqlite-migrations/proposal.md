# Proposal: Add Sqlite Migrations

## Change ID
`add-sqlite-migrations`

## Summary
Add rusqlite_migration to both SQLite databases with versioned schema migrations.

## Context
- Extends: messages.rs, reminders.rs, tools/schedule.rs

## Motivation
Current CREATE TABLE IF NOT EXISTS pattern breaks on ALTER TABLE. Need versioned migrations before any v4 schema changes.

## Requirements
### Req-1: Migration infrastructure
Add rusqlite_migration crate with PRAGMA user_version tracking to messages.db and schedules.db.

### Req-2: Initial migration
Convert existing CREATE TABLE IF NOT EXISTS statements to migration v1.

## Scope
- **IN**: rusqlite_migration integration, v1 migrations for existing tables
- **OUT**: New table schemas (those go in subsequent specs)

## Impact
| Area | Change |
|------|--------|
| Cargo.toml | Add rusqlite_migration dependency |
| messages.rs | Migration runner for messages.db |
| reminders.rs | Migration runner for reminders |
| tools/schedule.rs | Migration runner for schedules.db |

## Risks
| Risk | Mitigation |
|------|-----------|
| Scope expansion | Stick to PRD requirements, defer extras to backlog |
