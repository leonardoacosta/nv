# Proposal: Add Neon Management Tools

## Change ID
`add-neon-management-tools`

## Summary

Three read-only Neon REST API tools for infrastructure visibility: list projects, list branches
per project, and get compute endpoint status. Complements the existing `neon_query` tool (direct
SQL) with platform-level observability. Uses a single `NEON_API_KEY` env var against the
Neon API v2 (`https://console.neon.tech/api/v2/`).

## Context
- Extends: `crates/nv-daemon/src/tools/neon.rs` (existing Neon module), `crates/nv-daemon/src/tools/mod.rs` (tool registration + dispatch)
- Related: Existing `neon_query` tool (read-only SQL), aggregation layer (`project_health`)
- Auth: `NEON_API_KEY` env var — single API key covering all projects (distinct from per-project `POSTGRES_URL_{CODE}` used by `neon_query`)
- API: Neon API v2 at `https://console.neon.tech/api/v2/`

## Motivation

Nova already has `neon_query` for direct SQL against project databases. But there is no visibility
into the Neon platform itself — which projects exist, what branches are active, whether compute
endpoints are running or suspended. This matters for:

1. **Infrastructure inventory** — "What Neon projects do I have?" lists all projects with regions
2. **Branch awareness** — "What branches exist on OO's Neon project?" shows dev/preview branches
3. **Compute status** — "Is the OO database compute active or suspended?" shows endpoint state and size
4. **Aggregation layer input** — `project_health` can include Neon compute status (active/idle/suspended)
5. **Cost awareness** — knowing which computes are active helps estimate Neon billing

## Requirements

### Req-1: neon_projects Tool

```
neon_projects() -> Vec<ProjectSummary>
```

REST call: `GET https://console.neon.tech/api/v2/projects`

Returns all Neon projects with: name, ID, region, created_at. Formatted as an aligned table
for Telegram delivery. No parameters required.

### Req-2: neon_branches Tool

```
neon_branches(project_id: String) -> Vec<BranchSummary>
```

REST call: `GET https://console.neon.tech/api/v2/projects/{project_id}/branches`

Returns branches for a project with: name, ID, parent branch ID, created_at, current_state.
Formatted as aligned table. The `project_id` is the Neon project ID (e.g., `aged-bird-123456`),
not the Nova project code.

### Req-3: neon_compute Tool

```
neon_compute(project_id: String, branch_id: Option<String>) -> Vec<EndpointSummary>
```

REST call: `GET https://console.neon.tech/api/v2/projects/{project_id}/endpoints`

Returns compute endpoints with: ID, type (read_write/read_only), status (active/idle/suspended),
autoscaling size range, last_active timestamp. If `branch_id` is provided, filter results to
that branch only. Formatted as aligned table.

### Req-4: HTTP Client

Extend the existing `neon.rs` module with a `NeonApiClient` struct using `reqwest`:
- `Authorization: Bearer {NEON_API_KEY}` header on all requests
- 15s request timeout
- JSON response parsing into typed structs via serde
- Error mapping: 401 -> "Neon API key invalid", 404 -> "Project/branch not found", 429 -> "Rate limited"

### Req-5: Tool Registration

All three tools registered in `tools/mod.rs` alongside existing `neon_query`:
- Tool definitions with name, description, input schema
- Dispatch handlers in `execute_tool_send` and `execute_tool`
- Error handling for missing `NEON_API_KEY` env var

## Scope
- **IN**: Three read-only API tools (projects, branches, compute), HTTP client, serde types, error handling, Telegram formatting, humanize_tool entries
- **OUT**: Write operations (create/delete projects/branches), compute scaling controls, billing API, replacing `neon_query`

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tools/neon.rs` | Add NeonApiClient struct, 3 API methods, typed response structs, tool definitions |
| `crates/nv-daemon/src/tools/mod.rs` | Register 3 new tool definitions, add dispatch handlers |
| `crates/nv-daemon/src/orchestrator.rs` | Add humanize_tool entries for neon_projects, neon_branches, neon_compute |

## Risks
| Risk | Mitigation |
|------|-----------|
| NEON_API_KEY not set | Return clear error: "NEON_API_KEY env var not set — required for Neon management tools" |
| Rate limiting | Neon API allows 100 req/s — single-user scale is nowhere near this |
| Project ID confusion | Tool descriptions clarify this is the Neon project ID, not Nova project code |
| API response schema changes | Serde `#[serde(default)]` on optional fields prevents deserialization failures |
