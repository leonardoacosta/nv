# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/vercel.rs — module with typed structs: DeploymentSummary, BuildLog, BuildEvent [owner:api-engineer]
- [ ] [1.2] [P-1] Add VercelClient struct — holds reqwest::Client + token, constructed from VERCEL_TOKEN env var [owner:api-engineer]
- [ ] [1.3] [P-2] Add list_deployments(project: &str) async method — GET /v6/deployments, resolve project name to ID if needed [owner:api-engineer]
- [ ] [1.4] [P-2] Add get_build_logs(deploy_id: &str) async method — GET /v2/deployments/{id}/events, filter errors/warnings, truncate to 50 [owner:api-engineer]
- [ ] [1.5] [P-2] Add format_for_telegram() methods — state emoji (READY/ERROR/BUILDING), condensed deploy list, highlighted error lines [owner:api-engineer]
- [ ] [1.6] [P-3] Add resolve_project_id(name: &str) helper — GET /v9/projects/{name}, cache name->ID mapping in HashMap [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Add `mod vercel;` to main.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Register vercel_deployments, vercel_logs in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch handlers in tools.rs — validate inputs, call vercel module, return formatted result [owner:api-engineer]
- [ ] [2.4] [P-2] Add error handling — missing VERCEL_TOKEN, 401/404/429 HTTP errors, timeout [owner:api-engineer]
- [ ] [2.5] [P-3] Init VercelClient in main.rs on startup, pass to tool dispatch context [owner:api-engineer]
- [ ] [2.6] [P-3] Log each tool invocation to tool_usage audit table [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — parse deployment JSON fixtures, parse build log fixtures, error mapping [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "What's the latest deploy on otaku-odyssey?" via Telegram, verify formatted response [owner:api-engineer]
