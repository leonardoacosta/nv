# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Module

- [x] [1.1] [P-1] Create crates/nv-daemon/src/sentry_tools.rs — module with typed structs: SentryIssueSummary, SentryIssueDetail, StackFrame [owner:api-engineer]
- [x] [1.2] [P-1] Add SentryClient struct — holds reqwest::Client + token + org slug, constructed from SENTRY_AUTH_TOKEN and SENTRY_ORG env vars [owner:api-engineer]
- [x] [1.3] [P-2] Add list_issues(project: &str) async method — GET /api/0/projects/{org}/{project}/issues/, parse unresolved issues [owner:api-engineer]
- [x] [1.4] [P-2] Add get_issue(id: &str) async method — GET /api/0/issues/{id}/, fetch latest event for stack trace [owner:api-engineer]
- [x] [1.5] [P-2] Add format_stack_trace(frames: &[StackFrame]) helper — top 5 frames, skip vendor paths, condensed file:line format [owner:api-engineer]
- [x] [1.6] [P-2] Add format_for_telegram() methods — level emoji, event count badge, condensed issue list [owner:api-engineer]

## Tool Integration

- [x] [2.1] [P-1] Add `mod sentry_tools;` to main.rs [owner:api-engineer]
- [x] [2.2] [P-1] Register sentry_issues, sentry_issue in tools.rs tool definitions (name, description, input schema) [owner:api-engineer]
- [x] [2.3] [P-2] Add dispatch handlers in tools.rs — validate inputs (slug format, numeric ID), call sentry module, return formatted result [owner:api-engineer]
- [x] [2.4] [P-2] Add error handling — missing env vars, 401/403/404 HTTP errors, timeout, malformed JSON [owner:api-engineer]
- [x] [2.5] [P-3] Init SentryClient in main.rs on startup, pass to tool dispatch context [owner:api-engineer]
- [x] [2.6] [P-3] Log each tool invocation to tool_usage audit table [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] cargo test — parse issues list JSON fixture, parse issue detail fixture, stack trace formatting [owner:api-engineer]
- [ ] [3.4] [user] Manual test: send "Any Sentry errors on otaku-odyssey?" via Telegram, verify formatted response [owner:api-engineer]
