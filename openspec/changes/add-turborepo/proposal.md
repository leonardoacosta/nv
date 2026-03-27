# Proposal: Add Turborepo Task Runner

## Change ID
`add-turborepo`

## Summary
Add Turborepo as the monorepo task runner for NV, enabling parallel task execution, dependency-aware pipelines, and local caching across all 18 TypeScript workspace packages.

## Context
- Extends: root `package.json`, `.gitignore`
- Related: no prior task runner — builds are currently manual `pnpm --filter` or direct invocations

## Motivation
NV has 18 TypeScript packages across 3 workspace globs (`apps/*`, `packages/*`, `packages/tools/*`) with no task runner. Every build, typecheck, or lint run is manual and sequential. Adding Turborepo provides parallel execution with topological ordering, local caching to skip unchanged work, and a single entry point (`turbo build`) that respects the dependency graph. This is Tier 1 foundational infrastructure — every future spec benefits from faster, cacheable task execution.

## Requirements

### Req-1: turbo.json Task Pipeline
Define a root `turbo.json` with task definitions for `build`, `typecheck`, `lint`, `test`, and `dev`. Each task must declare correct `dependsOn`, `outputs`, `inputs`, and `cache` settings. The `build` task must topologically depend on upstream builds (`^build`) so that `@nova/db` compiles before packages that import it. The `dev` task must be persistent and uncached.

### Req-2: Root package.json Scripts
Add root-level scripts that delegate to Turborepo: `build`, `typecheck`, `lint`, `test`, `dev`, and `clean`. These provide the single entry point for all workspace-wide operations, replacing manual `pnpm --filter` invocations.

### Req-3: Dependency Graph Correctness
The task pipeline must respect the actual package dependency graph: `@nova/db` builds first (depended on by dashboard, daemon, cli), tool services build independently (no cross-deps), and `apps/dashboard` builds last (depends on `@nova/db`). Turbo's `^build` topological dependency handles this automatically via `workspace:*` references in each package.json.

### Req-4: Rust Crate Exclusion
Turborepo must not attempt to manage Rust crates in `crates/`. Since Turbo only operates on packages declared in `pnpm-workspace.yaml` and `crates/` is not listed there, no explicit exclusion config is needed — but this must be verified. Cargo remains the sole build system for Rust.

### Req-5: Cache and Gitignore Configuration
Add `.turbo/` to `.gitignore` to exclude local cache artifacts from version control. Configure local-only caching (no remote cache) — appropriate for a single-developer homelab project.

## Scope
- **IN**: `turbo.json`, root `package.json` scripts, `.gitignore` update, verification that Rust crates are unaffected
- **OUT**: remote caching setup (Vercel or self-hosted), CI/CD pipeline changes, per-package turbo.json overrides, Dockerfile changes, workspace restructuring

## Impact
| Area | Change |
|------|--------|
| Root config | New `turbo.json`, updated `package.json` and `.gitignore` |
| Developer workflow | `pnpm turbo build` / `pnpm build` replaces manual per-package builds |
| Build performance | Parallel execution + local caching — expect 2-5x speedup on incremental builds |
| Dependencies | New devDependency: `turbo` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Incorrect dependency graph causes build failures | Verify with `turbo build --dry-run --graph` before first real build |
| Tool services with no `build` script cause turbo errors | Turbo skips packages missing the requested script — no action needed |
| Cache invalidation misses cause stale builds | Configure `inputs` conservatively; `turbo clean` as escape hatch |
| Conflict with existing `pnpm --filter` workflows | Root scripts are additive — existing workflows continue to work |
