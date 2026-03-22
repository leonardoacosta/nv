# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [x] [1.1] [P-1] Create crates/nv-daemon/src/posthog_tools.rs — module with typed structs: TrendSeries, TrendsResponse, FeatureFlag, FeatureFlagsResponse [owner:api-engineer]
- [x] [1.2] [P-1] Add PostHog client helpers — build_client() with reqwest, api_key()/host() from env vars, resolve_project_id() mapping [owner:api-engineer]
- [x] [1.3] [P-2] Add query_trends(project: &str, event: &str) async method — POST /api/projects/{id}/insights/trend/, parse daily counts [owner:api-engineer]
- [x] [1.4] [P-2] Add list_flags(project: &str) async method — GET /api/projects/{id}/feature_flags/, filter active only [owner:api-engineer]
- [x] [1.5] [P-2] Add resolve_project_id(code: &str) helper — lookup project code (oo, tc) via POSTHOG_PROJECT_<CODE> or POSTHOG_PROJECT_ID env var [owner:api-engineer]
- [x] [1.6] [P-2] Add format_for_telegram() methods — daily breakdown with totals for trends, condensed flag list with rollout % [owner:api-engineer]

## Tool Integration

- [x] [2.1] [P-1] Add `mod posthog_tools;` to main.rs [owner:api-engineer]
- [x] [2.2] [P-1] Register posthog_trends, posthog_flags in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [x] [2.3] [P-2] Add dispatch handlers in tools.rs — validate project code, resolve ID, call posthog module, return formatted result [owner:api-engineer]
- [x] [2.4] [P-2] Add error handling — missing env vars, unknown project code, 401/404 HTTP errors, timeout [owner:api-engineer]
- [x] [2.5] [P-3] Init PostHogClient in main.rs on startup, pass to tool dispatch context [owner:api-engineer]
- [x] [2.6] [P-3] Log each tool invocation to tool_usage audit table [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes (0 posthog_tools issues) [owner:api-engineer]
- [x] [3.3] cargo test — 12 posthog tests pass: parse trends JSON, parse flags, project ID resolution, input validation, env var handling [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "How many signups on OO this week?" via Telegram, verify formatted response [owner:api-engineer]
