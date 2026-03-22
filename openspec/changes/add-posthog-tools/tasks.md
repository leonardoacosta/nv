# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/posthog.rs — module with typed structs: TrendResult, DayCount, FeatureFlag [owner:api-engineer]
- [ ] [1.2] [P-1] Add PostHogClient struct — holds reqwest::Client + API key + host URL + project ID mapping, constructed from env vars [owner:api-engineer]
- [ ] [1.3] [P-2] Add query_trends(project: &str, event: &str) async method — POST /api/projects/{id}/insights/trend/, parse daily counts [owner:api-engineer]
- [ ] [1.4] [P-2] Add list_flags(project: &str) async method — GET /api/projects/{id}/feature_flags/, filter active only [owner:api-engineer]
- [ ] [1.5] [P-2] Add resolve_project_id(code: &str) helper — lookup project code (oo, tc) in config mapping, return PostHog project ID [owner:api-engineer]
- [ ] [1.6] [P-2] Add format_for_telegram() methods — daily breakdown with totals for trends, condensed flag list with rollout % [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Add `mod posthog;` to main.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Register posthog_trends, posthog_flags in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch handlers in tools.rs — validate project code, resolve ID, call posthog module, return formatted result [owner:api-engineer]
- [ ] [2.4] [P-2] Add error handling — missing env vars, unknown project code, 401/404 HTTP errors, timeout [owner:api-engineer]
- [ ] [2.5] [P-3] Init PostHogClient in main.rs on startup, pass to tool dispatch context [owner:api-engineer]
- [ ] [2.6] [P-3] Log each tool invocation to tool_usage audit table [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — parse trends JSON fixture, parse flags fixture, project ID resolution [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "How many signups on OO this week?" via Telegram, verify formatted response [owner:api-engineer]
