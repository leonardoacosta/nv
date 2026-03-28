# Implementation Tasks

## DB Batch
(No DB tasks)

## API Batch
- [ ] [2.1] [P-1] Create Zod config schema in `packages/daemon/src/config/schema.ts` — daemon, agent, telegram, discord, teams, digest, autonomy, queue, database sections with transforms and refinements [owner:api-engineer]
- [ ] [2.2] [P-1] Create precedence resolver in `packages/daemon/src/config/resolver.ts` — load defaults, TOML, DB settings, env vars; merge in order; validate with Zod schema; throw formatted error on failure [owner:api-engineer]
- [ ] [2.3] [P-2] Rewrite `packages/daemon/src/config.ts` to delegate to schema + resolver — replace current ad-hoc loading with single `loadConfig()` call that returns validated config [owner:api-engineer]
- [ ] [2.4] [P-2] Add startup validation in `packages/daemon/src/index.ts` — fail-fast with exit code 1 on invalid config, log source breakdown on success, warn on deprecated keys [owner:api-engineer]
- [ ] [2.5] [P-3] Add `system.configSources` tRPC procedure in `packages/api/src/routers/system.ts` — return per-key source (toml/env/db/default), redact sensitive values to `***set***`/`***unset***` [owner:api-engineer]

## UI Batch
(No UI tasks)

## E2E Batch
- [ ] [4.1] [P-3] Test: valid config passes startup validation — supply complete config via TOML + env, assert daemon starts and logs source breakdown [owner:e2e-engineer]
- [ ] [4.2] [P-3] Test: missing required fields fail startup — omit `database.url`, assert exit code 1 and formatted error listing missing fields [owner:e2e-engineer]
- [ ] [4.3] [P-3] Test: precedence order respected — set `daemon.port` in TOML, DB, and env var with different values, assert env var wins [owner:e2e-engineer]
