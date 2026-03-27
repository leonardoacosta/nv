# task-pipeline Specification

## Purpose
TBD - created by archiving change add-turborepo. Update Purpose after archive.
## Requirements
### Requirement: Turborepo task pipeline configuration

The system SHALL provide a root `turbo.json` defining five tasks (`build`, `typecheck`, `lint`, `test`, `dev`) with correct dependency ordering, caching, and output declarations.

#### Scenario: Build task respects topological dependencies
Given the workspace has `@nova/db` as a dependency of `nova-dashboard`, `@nova/daemon`, and `@nova/cli`
When `turbo build` is executed
Then `@nova/db` builds before any package that depends on it
And all independent packages (tool services) build in parallel
And `nova-dashboard` builds after `@nova/db` completes

#### Scenario: Typecheck task depends on upstream builds
Given `@nova/db` must be built before downstream packages can typecheck against its types
When `turbo typecheck` is executed
Then `@nova/db` build runs first
And typecheck runs in parallel across all packages after their dependencies are built

#### Scenario: Dev task runs persistently without caching
Given a developer starts the dev environment
When `turbo dev` is executed
Then all `dev` scripts across packages run concurrently
And no cache is written or read for dev tasks
And the process remains running until terminated

#### Scenario: Lint task runs independently with no dependencies
Given lint does not depend on build output
When `turbo lint` is executed
Then lint runs in parallel across all packages with a lint script
And results are cached based on source file inputs

#### Scenario: Test task depends on build completion
Given tests may import built artifacts
When `turbo test` is executed
Then build completes first for packages with tests
And test runs in parallel across packages after their build

### Requirement: Root package.json provides workspace-wide script entry points

The root `package.json` MUST include scripts that delegate to Turborepo, providing a single command interface for all workspace operations.

#### Scenario: Developer runs workspace-wide build from root
Given the developer is at the repository root
When they run `pnpm build`
Then Turborepo executes the build task across all packages respecting the dependency graph

#### Scenario: Developer runs workspace-wide typecheck from root
Given the developer is at the repository root
When they run `pnpm typecheck`
Then Turborepo executes typecheck across all packages

#### Scenario: Clean removes Turbo cache
Given cached build artifacts exist in `.turbo/`
When the developer runs `pnpm clean`
Then the `.turbo/` directory is removed

### Requirement: Turbo cache artifacts excluded from git

The `.turbo/` directory MUST be added to `.gitignore` so local cache data is never committed.

#### Scenario: Cache directory is gitignored
Given Turborepo has been configured and builds have run
When `git status` is checked
Then `.turbo/` does not appear in untracked files

### Requirement: Rust crates unaffected by Turborepo

Turborepo MUST NOT operate on Rust crates. Only packages listed in `pnpm-workspace.yaml` are included in the task graph. The `crates/` directory remains managed exclusively by Cargo.

#### Scenario: Turbo build does not touch Rust crates
Given the workspace contains `crates/` with Rust code
When `turbo build` is executed
Then no Rust compilation occurs
And Cargo workspace in `Cargo.toml` is unchanged

