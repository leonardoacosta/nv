# Tasks: harden-jira-integration

## Dependencies

- jira-integration (MVP spec)

## Tasks

### Retry Logic

- [x] Implement `request_with_retry(max_retries, f)` on `JiraClient` in `crates/nv-daemon/src/jira/client.rs` -- exponential backoff starting 1s, doubling per attempt, max 3 retries. Only retry on 429/5xx/network errors. Respect `Retry-After` header on 429 if present [owner:api-engineer]
- [x] Wrap existing `jira_search`, `jira_get`, `jira_create`, `jira_transition`, `jira_assign`, `jira_comment` methods with `request_with_retry` -- pass max_retries=3, keep existing error handling for non-retryable errors [owner:api-engineer]

### Callback Handlers

- [x] Implement callback handler for `edit:{uuid}` in `crates/nv-daemon/src/callbacks.rs` -- load PendingAction by UUID, reply via Telegram asking what to change, set `editing_action_id` in agent loop state so next user message is treated as edit instruction, update payload, re-send draft with new inline keyboard [owner:api-engineer]
- [x] Implement callback handler for `cancel:{uuid}` in `crates/nv-daemon/src/callbacks.rs` -- load PendingAction by UUID, set status to `Cancelled`, edit original Telegram message with cancellation notice (e.g., "Cancelled: Create Bug on OO"), write updated state to `pending-actions.json` [owner:api-engineer]

### Expiry Sweep

- [x] Implement `check_expired_actions(telegram, state_path)` async fn -- scan `pending-actions.json` for PendingActions with status `Pending` and `created_at` older than 1 hour, mark as `Expired`, edit original Telegram message with expiry notice, write updated state [owner:api-engineer]
- [x] Wire `check_expired_actions` into agent loop periodic maintenance -- call on each loop tick or via a dedicated `tokio::time::interval` (e.g., every 5 minutes) [owner:api-engineer]

### Agent Loop Callback Routing

- [x] Wire callback routing in agent loop (`crates/nv-daemon/src/agent.rs`) -- when inbound message content starts with `[callback]`, parse callback data and route `approve:{uuid}` to existing handler, `edit:{uuid}` to new edit handler, `cancel:{uuid}` to new cancel handler. Pass through `nexus_err:` prefixed callbacks for Nexus handling [owner:api-engineer]

### Unit Tests (HTTP Mocks)

- [x] Add `wiremock` as dev-dependency to `crates/nv-daemon/Cargo.toml` [owner:api-engineer]
- [x] Test `handle_response` returns descriptive error for 401, 403, 404, 429 status codes -- mock HTTP server returns each status, verify error messages contain expected context strings [owner:api-engineer]
- [x] Test `request_with_retry` retries on 429 then succeeds -- mock returns 429 on first call, 200 on second, verify single retry and successful result [owner:api-engineer]
- [x] Test `request_with_retry` retries on 500/502/503 -- mock returns 5xx, verify retry attempts up to max then returns error [owner:api-engineer]
- [x] Test `request_with_retry` does NOT retry on 401/403/404 -- mock returns 401, verify immediate error without retry [owner:api-engineer]
- [x] Test transition name matching is case-insensitive -- mock `/transitions` endpoint returning `[{"id": "31", "name": "In Progress"}]`, verify "in progress", "IN PROGRESS", and "In Progress" all resolve to ID "31" [owner:api-engineer]
- [x] Test transition matching returns available transitions in error when no match found -- mock endpoint, request non-existent transition, verify error lists available names [owner:api-engineer]
- [x] Test PendingAction expiry sweep marks actions older than 1 hour as `Expired` -- create PendingAction with `created_at` 2 hours ago, run sweep, verify status changed and Telegram edit called [owner:api-engineer]

### Integration Tests

- [x] Create integration test (behind env var gate `NV_JIRA_INTEGRATION_TEST=1`): connect with real credentials, search issues with simple JQL (`project = OO ORDER BY created DESC`), verify at least response parsing succeeds [owner:api-engineer]
- [x] Create integration test: full create flow -- create issue on test project, verify `JiraCreatedIssue` returned with valid key, transition to "In Progress", add comment, verify via `jira_get` that comment exists [owner:api-engineer]

### Verify

- [x] `cargo build` passes for all workspace members
- [x] `cargo test -p nv-daemon` -- all new and existing unit tests pass
- [x] `cargo clippy` passes with no warnings
- [ ] Manual gate: "Create a P1 bug on OO" via Telegram -> draft shown with inline keyboard -> tap Edit -> reply with changed title -> new draft shown -> tap Approve -> issue exists in Jira. Then trigger another action -> tap Cancel -> cancelled notice shown. Verify expired actions show expiry notice after 1 hour [user]
