# Proposal: Add ADO CLI

## Change ID
`add-ado-cli`

## Summary
Add a standalone `ado` CLI binary (new crate `crates/ado-cli`) for Azure DevOps operations from the terminal. Surfaces the same ADO API client already in `nv-tools` through four subcommands: `pipelines`, `builds`, `work-items`, and `run-pipeline`.

## Context
- Extends: `crates/nv-tools/src/tools/ado.rs` — reuse `AdoClient`, `AdoPipeline`, `AdoBuild`, `AdoWorkItem` types and API methods
- Related: `crates/nv-cli` — existing pattern for a clap-based binary in this workspace
- Wave 5, Phase 2 (Tool Wrappers) — independent, no upstream dependencies

## Motivation
The ADO API integration exists as MCP tools inside `nv-tools` (used by Claude/daemon). Engineers also need to query pipeline status, browse work items, and trigger runs directly from their shell without going through an AI session. A thin CLI binary over the existing `AdoClient` delivers this with minimal new code.

## Requirements

### Req-1: `pipelines <project>`
List pipeline definitions for the given ADO project. Output: pipeline ID, name, folder (if set), one per line in a scannable table format.

#### Scenario: happy path
Given `ADO_ORG` and `ADO_PAT` are set, running `ado pipelines MyProject` prints a table of pipeline IDs and names.

#### Scenario: missing auth
Given `ADO_ORG` or `ADO_PAT` is absent, the command exits non-zero with a clear error message referencing the missing env var.

### Req-2: `builds <project>`
List recent builds across all pipelines for a project (last N builds per pipeline or latest N overall). Output includes build number, pipeline name, status, result, branch, requester, and relative timestamp.

#### Scenario: happy path
Running `ado builds MyProject` prints the most recent builds with status coloring (succeeded/failed/running).

#### Scenario: no recent builds
If no builds are found, print `No recent builds found for <project>` and exit 0.

### Req-3: `work-items <project> [--assigned-to <identity>]`
Query active work items in a project via WIQL. `--assigned-to @Me` filters to the current user (resolved via ADO identity). Output: ID, type, state, title, assignee, last-changed relative time.

#### Scenario: unfiltered
Running `ado work-items MyProject` returns all active (non-closed) work items up to a configurable max (default 50).

#### Scenario: assigned-to filter
Running `ado work-items MyProject --assigned-to @Me` returns only work items assigned to the authenticated user.

#### Scenario: @Me resolution
`@Me` is passed through as-is in the WIQL query — ADO resolves it server-side. No client-side identity lookup required.

### Req-4: `run-pipeline <project> <pipeline-id>`
Trigger a pipeline run via the ADO Pipelines API. Prints the run ID and web URL on success.

#### Scenario: happy path
Running `ado run-pipeline MyProject 42` queues a run and prints `Run #<run-id> queued: <url>`.

#### Scenario: permission error
If the PAT lacks queue permissions, exit non-zero with the API error message surfaced clearly.

### Req-5: Auth and Config
- PAT read from `ADO_PAT` env var (Doppler: `ADO_PAT`). AAD bearer token as future path — not in scope now.
- Organization read from `ADO_ORG` env var.
- No config file required for v1.

#### Scenario: env var precedence
Both vars are checked at command startup; missing vars produce an immediate error before any network call.

### Req-6: Output Format
- Default: human-readable text tables, relative timestamps (reuse `mod.rs::relative_time` from `nv-tools`).
- `--json` flag on all commands: raw JSON array output for scripting.

#### Scenario: --json flag
Running `ado pipelines MyProject --json` prints a JSON array of pipeline objects.

## Scope
- **IN**: `crates/ado-cli` binary, four subcommands, `--json` flag, reuse of `nv-tools` ADO client
- **OUT**: AAD/OAuth token flow, config file, web browser open, pagination beyond defaults, PR/branch operations, non-ADO CI providers

## Impact
| Area | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `crates/ado-cli` to `members` |
| `crates/ado-cli/` | New binary crate (Cargo.toml + src/) |
| `crates/nv-tools` | Extract `AdoClient` and types to `nv-core` or expose from `nv-tools` as lib dep |

## Risks
| Risk | Mitigation |
|------|-----------|
| `AdoClient` is not pub-accessible from a sibling crate | Add `ado-cli` → `nv-tools` dependency or move client to `nv-core` |
| `run-pipeline` requires write PAT scope | Document required PAT scopes in crate README |
| `relative_time` helper lives in `nv-tools::tools::mod` (private mod) | Re-export or inline the function in `ado-cli` |
