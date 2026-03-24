# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation (ado.rs)

- [x] [1.1] [P-1] Add AdoProject struct (id: String, name: String, state: String, last_update_time: Option<String>) + ProjectsResponse wrapper with serde derives [owner:api-engineer]
- [x] [1.2] [P-1] Add projects() method to AdoClient — GET {org_url}/_apis/projects?api-version=7.0, deserialize into Vec<AdoProject> [owner:api-engineer]
- [x] [1.3] [P-2] Add format_projects(projects: &[AdoProject]) — header "Projects (N):", each line "name (state) — last updated YYYY-MM-DD", empty case returns "(no projects found)" [owner:api-engineer]
- [x] [1.4] [P-2] Add pub async fn ado_projects() entry point — AdoClient::from_env(), call projects(), format, log with tracing::info [owner:api-engineer]

## Tool Integration (mod.rs + orchestrator.rs)

- [x] [2.1] [P-1] Add ado_projects to ado_tool_definitions() — no-param input schema, description mentions discovering projects before using ado_pipelines/ado_builds [owner:api-engineer]
- [x] [2.2] [P-2] Add "ado_projects" dispatch arm in execute_tool() and execute_tool_send() — call ado_tools::ado_projects().await, return ToolResult::Immediate [owner:api-engineer]
- [x] [2.3] [P-3] Add "ado_projects" to humanize_tool() match arm alongside existing ado_pipelines/ado_builds entry [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — format_projects empty + populated cases, ado_projects in tool name assertions [owner:api-engineer]
- [ ] [3.4] [user] Manual test: ask Nova "What ADO projects do we have?" via Telegram, verify formatted project list [owner:api-engineer]
