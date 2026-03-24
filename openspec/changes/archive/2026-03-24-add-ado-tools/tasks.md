# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [x] [1.1] [P-1] Create crates/nv-daemon/src/ado_tools.rs — AdoClient struct with org_url + pat + reqwest::Client, from_env() constructor with Basic auth header [owner:api-engineer]
- [x] [1.2] [P-1] Add pipelines(project: &str) method — GET /{project}/_apis/pipelines?api-version=7.1, deserialize into Vec<AdoPipeline>, cap at 50 [owner:api-engineer]
- [x] [1.3] [P-2] Add builds(project: &str, pipeline_id: u32) method — GET /{project}/_apis/build/builds?definitions={id}&$top=10&api-version=7.1, deserialize into Vec<AdoBuild> [owner:api-engineer]
- [x] [1.4] [P-2] Add format_pipelines(pipelines: &[AdoPipeline]) helper — formatted list with id, name, folder [owner:api-engineer]
- [x] [1.5] [P-2] Add format_builds(builds: &[AdoBuild]) helper — formatted list with buildNumber, status, result, branch, requestedFor, timestamps [owner:api-engineer]
- [x] [1.6] [P-3] Add mod ado_tools declaration in main.rs [owner:api-engineer]

## Tool Integration

- [x] [2.1] [P-1] Register ado_pipelines tool in register_tools() — input schema: { project: string } [owner:api-engineer]
- [x] [2.2] [P-1] Register ado_builds tool in register_tools() — input schema: { project: string, pipeline_id: integer } [owner:api-engineer]
- [x] [2.3] [P-2] Add dispatch cases in execute_tool() for both tools — call AdoClient methods, format output [owner:api-engineer]
- [x] [2.4] [P-2] AdoClient constructed from ADO_ORG + ADO_PAT env vars — graceful fallback if missing [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — new AdoClient tests (format_pipelines, format_builds, missing env) + existing tests pass [owner:api-engineer]
- [ ] [3.4] [user] Manual test: ask Nova "What pipelines are on ProjectX?" via Telegram, verify formatted pipeline list [owner:api-engineer]
