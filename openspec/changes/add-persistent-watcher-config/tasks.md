# Implementation Tasks

## DB Batch
(No DB tasks)

## API Batch
- [ ] [2.1] [P-1] Create `packages/daemon/src/features/settings/persistent-settings.ts` — readSettings (SELECT watcher.*/briefing.* from settings table), writeSettings (upsert key-value pairs), mergeOverToml (DB values override TOML defaults) [owner:api-engineer]
- [ ] [2.2] [P-2] Update `packages/api/src/routers/automation.ts` — updateWatcher and updateSettings procedures call writeSettings before updating in-memory config; wrap in transaction so memory only updates on DB success [owner:api-engineer]
- [ ] [2.3] [P-2] Add startup hydration in `packages/daemon/src/index.ts` — after TOML load, call readSettings + mergeOverToml; log overridden keys at INFO; fall back to TOML with WARN on DB error [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [ ] [4.1] Test: watcher config persists across daemon restart — PATCH interval_minutes, restart daemon, GET watcher config, assert new value retained [owner:e2e-engineer]
