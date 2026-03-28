# Proposal: Add Persistent Watcher Config

## Change ID
`add-persistent-watcher-config`

## Summary

Persist watcher and automation configuration to the database settings table. Currently, PATCH
updates to watcher config (enabled, interval_minutes, quiet hours) are stored as in-memory
overrides that revert on daemon restart. The settings table exists in the DB but is not used
for watcher configuration.

## Context

- Extends: `packages/api/src/routers/automation.ts` (getWatcher/updateWatcher procedures)
- Extends: `packages/daemon/src/index.ts` (startup hydration)
- New: `packages/daemon/src/features/settings/persistent-settings.ts`
- The `settings` table in the DB stores key-value pairs but is not connected to watcher config
- Dashboard automation page allows users to toggle watcher, change interval, set quiet hours
- All changes are lost on daemon restart

## Motivation

Leo configures watcher settings via the dashboard, daemon restarts (deploy, crash, manual),
settings revert to TOML defaults. This is silently confusing -- the dashboard still shows the
old values until it re-fetches. A 5-minute fix costs zero and eliminates a recurring friction
point.

## Requirements

### Req-1: Settings persistence layer

Create `packages/daemon/src/features/settings/persistent-settings.ts`:

- Read settings from DB on startup: `SELECT key, value FROM settings WHERE key LIKE 'watcher.%'`
- Merge DB values over TOML config (DB takes precedence)
- On update: write to DB AND update in-memory config
- Supported keys: `watcher.enabled`, `watcher.interval_minutes`, `watcher.quiet_start`,
  `watcher.quiet_end`, `watcher.prompt`, `briefing.hour`, `briefing.prompt`

### Req-2: Update automation router

In `packages/api/src/routers/automation.ts`:

- `updateWatcher`: write to settings table, then update in-memory
- `updateSettings`: same pattern
- `getWatcher`: read from in-memory (already DB-hydrated at startup)

### Req-3: Startup hydration

In daemon startup (`packages/daemon/src/index.ts`):

- After loading TOML config, query settings table
- Override matching config keys with DB values
- Log which settings were overridden from DB (INFO level)

## Scope

- **IN**: `packages/daemon/src/features/settings/` (new),
  `packages/api/src/routers/automation.ts` (modified),
  `packages/daemon/src/index.ts` (startup hydration)
- **OUT**: TOML config parsing (unchanged, still used as defaults), dashboard UI (unchanged)

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/src/features/settings/persistent-settings.ts` | New -- DB read/write for settings |
| `packages/api/src/routers/automation.ts` | Modified -- persist on update |
| `packages/daemon/src/index.ts` | Extended -- hydrate from DB at startup |

## Risks

| Risk | Mitigation |
|------|-----------|
| DB unavailable at startup | Fall back to TOML values with WARN log |
| Schema mismatch (TOML key names vs DB keys) | Use consistent dot-notation keys |
| Stale in-memory state after DB write fails | Wrap in transaction, only update memory on success |
