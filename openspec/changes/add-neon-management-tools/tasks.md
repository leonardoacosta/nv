# Implementation Tasks

<!-- beads:epic:TBD -->

## API Client & Types

- [x] [1.1] [P-1] Add NeonApiClient struct to neon.rs — holds reqwest::Client + NEON_API_KEY token, constructed from env var [owner:api-engineer]
- [x] [1.2] [P-1] Add typed response structs — ProjectSummary, BranchSummary, EndpointSummary with serde Deserialize + defaults on optional fields [owner:api-engineer]
- [x] [1.3] [P-2] Add list_projects() async method — GET /projects, parse response, return Vec<ProjectSummary> [owner:api-engineer]
- [x] [1.4] [P-2] Add list_branches(project_id) async method — GET /projects/{id}/branches, parse response, return Vec<BranchSummary> [owner:api-engineer]
- [x] [1.5] [P-2] Add list_endpoints(project_id, branch_id?) async method — GET /projects/{id}/endpoints, optional branch_id filter, return Vec<EndpointSummary> [owner:api-engineer]
- [x] [1.6] [P-2] Add format methods for each result type — aligned table formatting consistent with existing format_results() pattern [owner:api-engineer]

## Tool Registration & Dispatch

- [x] [2.1] [P-1] Add neon_projects, neon_branches, neon_compute to neon_tool_definitions() in neon.rs [owner:api-engineer]
- [x] [2.2] [P-1] Add dispatch handlers for all 3 tools in execute_tool_send in tools/mod.rs [owner:api-engineer]
- [x] [2.3] [P-1] Add dispatch handlers for all 3 tools in execute_tool in tools/mod.rs [owner:api-engineer]
- [x] [2.4] [P-2] Add humanize_tool entries in orchestrator.rs for neon_projects, neon_branches, neon_compute [owner:api-engineer]
- [x] [2.5] [P-2] Update register_tools_returns_expected_count test — bump count from 84 to 87, assert new tool names [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — NeonApiClient::from_env missing key, response struct deserialization with fixture JSON [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "What Neon projects do I have?" via Telegram, verify formatted response [owner:api-engineer]
