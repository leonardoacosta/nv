# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Add jira_webhook_secret (Option<String>) to JiraConfig in nv-core config.rs [owner:api-engineer]
- [ ] [2.2] [P-1] Create crates/nv-daemon/src/jira/webhooks.rs — Jira webhook payload types with serde: WebhookEvent (webhookEvent string, timestamp, issue, comment, changelog), IssueEvent fields (issue key, summary, status name, assignee display_name, priority name), ChangelogItem (field, fromString, toString), CommentEvent fields (comment author display_name, body, created) [owner:api-engineer]
- [ ] [2.3] [P-1] Add webhook secret validation function in webhooks.rs — extract secret from query param or header, compare against config, return 401 on mismatch [owner:api-engineer]
- [ ] [2.4] [P-1] Add event routing in webhooks.rs — match on webhookEvent field: "jira:issue_updated" -> handle_issue_updated, "jira:issue_created" -> handle_issue_created, "comment_created" -> handle_comment_created, unknown -> log and 200 OK [owner:api-engineer]
- [ ] [2.5] [P-1] Implement handle_issue_updated — extract changelog items, filter for relevant fields (status, assignee, priority), format Telegram alert message, update memory [owner:api-engineer]
- [ ] [2.6] [P-1] Implement handle_issue_created — extract issue key/summary/assignee, format Telegram alert, write memory entry [owner:api-engineer]
- [ ] [2.7] [P-1] Implement handle_comment_created — extract issue key, comment author, body preview (first 200 chars), format Telegram alert [owner:api-engineer]
- [ ] [2.8] [P-2] Add memory update helper — write/append to ~/.nv/memory/ with timestamped entry reflecting external Jira change [owner:api-engineer]
- [ ] [2.9] [P-2] Add POST /webhooks/jira route to http.rs — wire handler with shared state (config, TelegramClient, memory path) [owner:api-engineer]
- [ ] [2.10] [P-2] Re-export webhooks module from crates/nv-daemon/src/jira/mod.rs [owner:api-engineer]
- [ ] [2.11] [P-2] Wire webhook route into axum router in main.rs [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit tests for webhook payload serde: deserialize issue_updated, issue_created, comment_created from JSON fixtures [owner:test-writer]
- [ ] [3.4] Unit test for secret validation: valid secret -> pass, missing secret -> 401, wrong secret -> 401 [owner:test-writer]
- [ ] [3.5] Unit tests for event routing: verify correct handler called per webhookEvent string, unknown event returns 200 [owner:test-writer]
- [ ] [3.6] Unit tests for Telegram alert formatting: issue updated (status change), issue created, comment created [owner:test-writer]
- [ ] [3.7] cargo test — all new + existing tests pass [owner:api-engineer]
