# Capability: Sqlite Migrations

## ADDED Requirements

### Requirement: SQLite migration infrastructure
The daemon MUST use rusqlite_migration for all SQLite schema management with PRAGMA user_version tracking.

#### Scenario: Fresh database initialization
**Given** no database file exists
**When** the daemon starts
**Then** all migrations run sequentially and PRAGMA user_version reflects the latest version

#### Scenario: Existing database upgrade
**Given** a database at version 0 (pre-migration)
**When** the daemon starts with migration v1 defined
**Then** migration v1 runs and PRAGMA user_version is set to 1
