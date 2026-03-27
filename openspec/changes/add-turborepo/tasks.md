# Implementation Tasks

<!-- beads:epic:nv-7k32 -->

## DB Batch

(no database changes)

## API Batch

- [x] [2.1] [P-1] Install `turbo` as root devDependency: `pnpm add -D turbo -w` [owner:api-engineer] [beads:nv-ggch]
- [x] [2.2] [P-1] Create root `turbo.json` with task pipeline: build (dependsOn ^build, outputs dist/**//.next/**), typecheck (dependsOn ^build, no outputs), lint (no deps, no outputs), test (dependsOn build), dev (cache false, persistent true) [owner:api-engineer] [beads:nv-xmqo]
- [x] [2.3] [P-1] Add root `package.json` scripts: build, typecheck, lint, test, dev (all delegating to `turbo run <task>`), plus clean (`rm -rf .turbo`) [owner:api-engineer] [beads:nv-v8m9]
- [x] [2.4] [P-1] Add `.turbo/` to `.gitignore` [owner:api-engineer] [beads:nv-bsfn]
- [x] [2.5] [P-2] Verify `turbo build --dry-run` produces correct topological order: @nova/db first, tool services parallel, dashboard last [owner:api-engineer] [beads:nv-ujme]
- [x] [2.6] [P-2] Verify `turbo build --graph` confirms Rust crates/ are not included in the task graph [owner:api-engineer] [beads:nv-7nh6]
- [x] [2.7] [P-2] Run `turbo build` end-to-end: 17/18 succeeded, nova-dashboard fails on pre-existing type error (missing updateProjectSchema/createProjectSchema exports from @nova/db — unrelated to Turbo) [owner:api-engineer] [beads:nv-5o4h]
- [x] [2.8] [P-2] Run `turbo typecheck` end-to-end: 17/18 packages passed, nova-dashboard fails on same pre-existing type error (missing updateProjectSchema/createProjectSchema from @nova/db) [owner:api-engineer] [beads:nv-r16o]

## UI Batch

(no UI changes)

## E2E Batch

(no E2E tests for infrastructure-only change)
