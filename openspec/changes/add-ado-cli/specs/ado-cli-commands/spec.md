# Spec: ADO CLI Commands

## ADDED Requirements

### Requirement: Binary Crate `crates/ado-cli`

The workspace MUST include a new Cargo binary crate `crates/ado-cli` that produces a binary named `ado`. It SHALL depend on `nv-tools` (for `AdoClient`, ADO types, and `relative_time`), `clap`, `tokio`, `serde_json`, and `anyhow`. The `nv-tools` lib MUST re-export `AdoClient`, `AdoPipeline`, `AdoBuild`, `AdoWorkItem`, and the `relative_time` helper so `ado-cli` can use them. `crates/ado-cli` SHALL be added to `[workspace].members` in the root `Cargo.toml`.

Auth: the binary MUST read `ADO_ORG` and `ADO_PAT` from environment variables at startup and exit 1 with a clear error message if either is missing, before making any network call.

#### Scenario: binary builds
Given the `crates/ado-cli` directory exists with all source files
When `cargo build -p ado-cli` is run
Then the build succeeds and produces `./target/debug/ado`

#### Scenario: missing ADO_ORG
Given `ADO_ORG` env var is not set
When any `ado` subcommand is invoked
Then stderr contains "Azure DevOps not configured — ADO_ORG env var not set"
And the process exits with code 1

#### Scenario: missing ADO_PAT
Given `ADO_PAT` env var is not set
When any `ado` subcommand is invoked
Then stderr contains "Azure DevOps not configured — ADO_PAT env var not set"
And the process exits with code 1

### Requirement: `ado pipelines <project>`

The `pipelines` subcommand SHALL list ADO pipeline definitions for the given project by calling `AdoClient::pipelines(project)`. Text output MUST include columns `ID | NAME | FOLDER`. With `--json` flag the output MUST be a JSON array of `{ id, name, folder }` objects.

#### Scenario: pipelines listed
Given `ADO_ORG` and `ADO_PAT` are set and the project exists
When `ado pipelines Acme` is run
Then stdout prints a table with one row per pipeline showing ID, name, and folder

#### Scenario: --json output
Given `ADO_ORG` and `ADO_PAT` are set
When `ado pipelines Acme --json` is run
Then stdout is a valid JSON array parseable with `jq '.[0].id'`

### Requirement: `ado builds <project>`

The `builds` subcommand SHALL list the most recent builds for a project (top 20, no pipeline filter). Output columns MUST include `BUILD | PIPELINE | STATUS | RESULT | BRANCH | BY | WHEN`. The WHEN column MUST use `relative_time` formatting.

#### Scenario: builds listed
Given builds exist for the project
When `ado builds Acme` is run
Then stdout prints a table of recent builds with relative timestamps

#### Scenario: no recent builds
Given no builds exist for the project
When `ado builds EmptyProject` is run
Then stdout contains "No recent builds found for EmptyProject."
And the process exits with code 0

### Requirement: `ado work-items <project> [--assigned-to <identity>]`

The `work-items` subcommand SHALL query active work items via WIQL. The default query MUST return all non-closed items ordered by `System.ChangedDate DESC` up to 50. With `--assigned-to <identity>` the WIQL WHERE clause MUST append `AND [System.AssignedTo] = '<identity>'`. The literal `@Me` SHALL be passed as-is to ADO for server-side resolution. Output columns MUST include `ID | TYPE | STATE | TITLE | ASSIGNED TO | CHANGED`.

#### Scenario: unfiltered work items
Given work items exist in the project
When `ado work-items Acme` is run
Then stdout prints a table of non-closed work items up to 50 rows

#### Scenario: assigned-to filter
Given work items assigned to the authenticated user exist
When `ado work-items Acme --assigned-to @Me` is run
Then stdout prints only work items assigned to the current user

#### Scenario: no results
Given no matching work items exist
When `ado work-items Acme --assigned-to @Me` is run
Then stdout contains "No work items found."
And the process exits with code 0

### Requirement: `ado run-pipeline <project> <pipeline-id>`

The `run-pipeline` subcommand SHALL trigger a pipeline run via `POST /_apis/pipelines/<id>/runs?api-version=7.1`. On success it MUST print the run ID and the `_links.web.href` URL. On API error it MUST write the error message to stderr and exit non-zero.

#### Scenario: run queued
Given valid PAT with queue permissions and a valid pipeline ID
When `ado run-pipeline Acme 42` is run
Then stdout contains "Run #<run-id> queued: <url>"

#### Scenario: permission error surfaced
Given PAT lacks queue permission
When `ado run-pipeline Acme 42` is run
Then stderr contains the API error message including the HTTP status
And the process exits with code 1

### Requirement: Global `--json` Flag

All four subcommands SHALL accept a `--json` flag. When set, the output MUST be a JSON value (array or object) to stdout with no decorative text. Errors MUST always be written to stderr as plain text regardless of `--json`.

#### Scenario: json flag works across subcommands
Given `--json` is passed to `pipelines`, `builds`, `work-items`, or `run-pipeline`
When the command succeeds
Then stdout is valid JSON and contains no table formatting or decorative headers
