# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/github.rs — module with typed structs: PrSummary, RunSummary, IssueSummary [owner:api-engineer]
- [ ] [1.2] [P-1] Add exec_gh(args: &[&str]) async helper — runs `gh` via tokio::process::Command, captures stdout/stderr, 15s timeout [owner:api-engineer]
- [ ] [1.3] [P-2] Add parse_pr_list(json: &str) -> Vec<PrSummary> — deserialize gh pr list --json output [owner:api-engineer]
- [ ] [1.4] [P-2] Add parse_run_status(json: &str) -> Vec<RunSummary> — deserialize gh run list --json output [owner:api-engineer]
- [ ] [1.5] [P-2] Add parse_issues(json: &str) -> Vec<IssueSummary> — deserialize gh issue list --json output [owner:api-engineer]
- [ ] [1.6] [P-2] Add format_for_telegram() methods on each struct — condensed output with status emoji [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Add `mod github;` to main.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Register gh_pr_list, gh_run_status, gh_issues in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch handlers in tools.rs — validate repo format (owner/repo), call github module, return formatted result [owner:api-engineer]
- [ ] [2.4] [P-2] Add input validation — repo param must match `owner/repo` regex pattern [owner:api-engineer]
- [ ] [2.5] [P-2] Add error handling — gh not found, auth expired, timeout, malformed JSON [owner:api-engineer]
- [ ] [2.6] [P-3] Log each tool invocation to tool_usage audit table (name, input summary, success, duration_ms) [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — parse_pr_list, parse_run_status, parse_issues with fixture JSON [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "List PRs on nyaptor/nv" via Telegram, verify formatted response [owner:api-engineer]
