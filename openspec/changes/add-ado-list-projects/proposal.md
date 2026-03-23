# Proposal: Add ADO List Projects Tool

## Change ID
`add-ado-list-projects`

## Summary

New `ado_projects` tool that lists all Azure DevOps projects in the configured org. Fills a gap
where the two existing ADO tools (`ado_pipelines`, `ado_builds`) both require a project name but
Nova has no way to discover available projects.

## Context
- Extends: `crates/nv-daemon/src/tools/ado.rs` (ADO client + tool definitions)
- Extends: `crates/nv-daemon/src/tools/mod.rs` (dispatch in `execute_tool` + `execute_tool_send`)
- Extends: `crates/nv-daemon/src/orchestrator.rs` (`humanize_tool` function)
- Related: Archived `add-ado-tools` spec (original ADO integration)
- Auth: `ADO_ORG` + `ADO_PAT` env vars (already configured and healthy)

## Motivation

Nova has `ado_pipelines(project)` and `ado_builds(project, pipeline_id)` but no way to list
available projects. When asked "What ADO projects do we have?", Nova must admit it cannot answer.
The health check already calls `/_apis/projects` successfully — we just need to expose that
as a tool with response parsing and formatting.

## Requirements

### Req-1: AdoClient.projects() Method

Add `projects()` method to the existing `AdoClient`:
- Endpoint: `GET {org_url}/_apis/projects?api-version=7.0`
- Response shape: `{"value": [{"id": "...", "name": "...", "state": "wellFormed", "lastUpdateTime": "..."}]}`
- Deserialize into `Vec<AdoProject>` with fields: id, name, state, last_update_time
- No input parameters (org comes from the client config)

### Req-2: Formatting

`format_projects(projects: &[AdoProject])` helper:
- Format each project as: `name (state) — last updated YYYY-MM-DD`
- Header: `Projects (N):`
- Empty case: `(no projects found)`

### Req-3: Tool Definition

Add `ado_projects` to `ado_tool_definitions()`:
- Name: `ado_projects`
- Description: "List all Azure DevOps projects in the configured organization. Returns project name, state, and last update date. Use this to discover available projects before calling ado_pipelines or ado_builds."
- Input schema: `{"type": "object", "properties": {}}` (no parameters)

### Req-4: Dispatch + Humanize

- Add `"ado_projects"` dispatch arm in both `execute_tool` and `execute_tool_send` in `mod.rs`
- Add `"ado_projects"` to the existing `"ado_pipelines" | "ado_builds"` match arm in `humanize_tool`

## Scope
- **IN**: AdoProject struct, projects() method, format_projects(), tool definition, dispatch, humanize entry, unit tests
- **OUT**: Filtering, pagination, project creation/deletion, caching

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/ado.rs` | Add AdoProject struct, ProjectsResponse, projects() method, format_projects(), ado_projects() entry point, tool def, tests |
| `crates/nv-daemon/src/tools/mod.rs` | Add dispatch arm in execute_tool + execute_tool_send, add to tool name test |
| `crates/nv-daemon/src/orchestrator.rs` | Add `"ado_projects"` to humanize_tool match arm |

## Risks
| Risk | Mitigation |
|------|-----------|
| Large number of projects | Unlikely at single-org scale; no pagination needed |
| API version mismatch | Using stable 7.0; health check already validates endpoint |
