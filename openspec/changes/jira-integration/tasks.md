# Tasks: jira-integration

## Dependencies

- agent-loop (spec-4)
- memory-system (spec-5)

## Tasks

### Jira API Types

- [x] Create `crates/nv-daemon/src/jira/types.rs` with request types: `JiraCreateParams` (`project`, `issue_type`, `title`, `description`, `priority`, `assignee_account_id`, `labels`) — `#[derive(Debug, Serialize)]`
- [x] Add response types: `JiraSearchResult` (`total`, `max_results`, `issues`), `JiraIssue` (`id`, `key`, `fields`), `JiraIssueFields` (`summary`, `status`, `assignee`, `priority`, `issuetype`, `project`, `labels`, `created`, `updated`, `description`, `comment`) — `#[derive(Debug, Deserialize)]`
- [x] Add field types: `JiraStatus` (`name`, `id`), `JiraUser` (`account_id`, `display_name` with serde rename), `JiraPriority` (`name`, `id`), `JiraIssueType` (`name`), `JiraProject` (`key`, `name`) — `#[derive(Debug, Deserialize)]`
- [x] Add `JiraCreatedIssue` (`id`, `key`, `self_url` with serde rename), `JiraTransitionsResponse` (`transitions`), `JiraTransition` (`id`, `name`), `JiraComment` (`id`, `body`, `created`, `author`), `JiraCommentPage` (`total`, `comments`) — `#[derive(Debug, Deserialize)]`
- [x] Add `JiraPendingAction` (`id`, `action_type`, `description`, `payload`, `status`, `created_at`, `telegram_message_id`), `JiraActionType` enum (`Create`, `Transition`, `Assign`, `Comment`), `PendingActionStatus` enum (`Pending`, `Approved`, `Cancelled`, `Expired`) — `#[derive(Debug, Serialize, Deserialize)]`

### JiraClient Struct and Auth

- [x] Create `crates/nv-daemon/src/jira/mod.rs` with `JiraClient` struct: `http: reqwest::Client`, `base_url: String`, `auth_email: String`, `auth_token: String`
- [x] Implement `JiraClient::new(instance_url, email, api_token)` — build reqwest client with default headers: `Authorization: Basic base64(email:token)`, `Accept: application/json`, `Content-Type: application/json`, 30s timeout
- [x] Implement `handle_response<T: DeserializeOwned>(resp, context)` — check status, parse 401/403/404/429/5xx into descriptive error messages
- [ ] Implement `request_with_retry(max_retries, f)` — exponential backoff starting 1s, doubling per attempt, only retry on 429/5xx/network errors, max 3 retries [deferred]

### Read Tools (No Confirmation)

- [x] Implement `jira_search(jql)` — POST `/rest/api/3/search` with `maxResults: 50`, fields list (`summary`, `status`, `assignee`, `priority`, `issuetype`, `project`, `labels`, `created`, `updated`, `description`), return `Vec<JiraIssue>`
- [x] Implement `jira_get(issue_key)` — GET `/rest/api/3/issue/{key}` with fields query param including `comment`, return `JiraIssue`
- [x] Implement `search_users(query)` — GET `/rest/api/3/user/search` with query param, maxResults 10, return `Vec<JiraUser>` for assignee resolution

### Write Tools (Require Confirmation)

- [x] Implement `jira_create(params: &JiraCreateParams)` — POST `/rest/api/3/issue` with fields object; description uses Atlassian Document Format (ADF doc → paragraph → text); priority by name; assignee by accountId; return `JiraCreatedIssue`
- [x] Implement `jira_transition(issue_key, transition_name)` — GET `/rest/api/3/issue/{key}/transitions` to list available transitions, case-insensitive match by name, POST transition with matched ID; error lists available transitions on mismatch
- [x] Implement `jira_assign(issue_key, assignee_account_id)` — PUT `/rest/api/3/issue/{key}/assignee` with `accountId` body
- [x] Implement `jira_comment(issue_key, comment_body)` — POST `/rest/api/3/issue/{key}/comment` with ADF body format, return `JiraComment`

### PendingAction Confirmation Flow

- [x] Implement `create_pending_action(tool_name, input)` — generate UUID, map tool name to `JiraActionType`, build human-readable description string, serialize payload, set status `Pending`, set `created_at` to now
- [x] Implement `save_pending_action(action)` — read `~/.nv/state/pending-actions.json`, append action, write back; create file if missing
- [x] Implement `load_pending_action(id)` — read pending-actions.json, find by UUID, return `Option<JiraPendingAction>`
- [x] Implement `send_confirmation_keyboard(telegram, action)` — format draft summary (project, type, priority, title, description, assignee, labels for create; issue_key + target status for transition; etc.), send via Telegram with `InlineKeyboard::confirm_action(action.id)`, store returned `message_id` on the PendingAction
- [x] Implement callback handler for `approve:{uuid}` — load PendingAction, match `action_type`, deserialize payload, execute corresponding JiraClient method, edit Telegram message with result, update PendingAction status to `Approved`, write memory entry
- [ ] Implement callback handler for `edit:{uuid}` — load PendingAction, reply asking what to change, agent loop processes user response to update payload, re-send draft with new keyboard [deferred]
- [ ] Implement callback handler for `cancel:{uuid}` — load PendingAction, set status to `Cancelled`, edit Telegram message with cancellation notice, remove from pending list [deferred]
- [ ] Implement expiry sweep — on agent loop tick, check pending actions older than 1 hour, mark as `Expired`, edit Telegram message with expiry notice [deferred]

### Agent Tool Registration

- [x] Create `crates/nv-daemon/src/jira/tools.rs` with `jira_tool_definitions()` returning `Vec<serde_json::Value>` — 6 tool definitions (`jira_search`, `jira_get`, `jira_create`, `jira_transition`, `jira_assign`, `jira_comment`) with name, description, and `input_schema`
- [x] Implement `format_issues_for_claude(issues)` — format Vec<JiraIssue> as concise text for Claude tool result (key, summary, status, assignee, priority per issue)
- [x] Implement `format_issue_for_claude(issue)` — format single JiraIssue as detailed text including comments

### Agent Loop Integration

- [x] Add tool dispatch in agent loop for `jira_search` — call `jira_client.jira_search(jql)`, return formatted issues as tool result (immediate, no confirmation)
- [x] Add tool dispatch for `jira_get` — call `jira_client.jira_get(issue_key)`, return formatted issue as tool result (immediate, no confirmation)
- [x] Add tool dispatch for `jira_create`, `jira_transition`, `jira_assign`, `jira_comment` — create PendingAction, save to state, send Telegram confirmation keyboard, return "Awaiting confirmation" as tool result
- [ ] Wire callback query handling: when agent loop receives `[callback] approve:{uuid}` / `edit:{uuid}` / `cancel:{uuid}`, route to corresponding PendingAction handler [deferred]

### Daemon Integration

- [x] Add `mod jira;` to nv-daemon, create `jira/` module directory
- [x] Add `base64` crate dependency to nv-daemon Cargo.toml
- [x] Update config struct: add `jira` section with `instance_url: String`, `default_project: Option<String>`
- [x] Update secrets struct: add `jira_email: Option<String>`, `jira_api_token: Option<String>` from `NV_JIRA_EMAIL` and `NV_JIRA_API_TOKEN` env vars
- [x] Update `main.rs`: conditionally create `JiraClient` if jira config + secrets present, pass into agent loop; log warning if jira not configured
- [x] Update `config/nv.example.toml`: add `[jira]` section with `instance_url` and `default_project`

### Unit Tests

- [x] Test: `JiraCreateParams` serializes correctly with all optional fields present
- [x] Test: `JiraCreateParams` serializes correctly with only required fields
- [x] Test: `JiraIssue` deserializes from sample Jira API v3 JSON response
- [x] Test: `JiraSearchResult` deserializes with empty issues array
- [x] Test: `JiraTransitionsResponse` deserializes with multiple transitions
- [x] Test: `JiraUser` deserializes with serde rename (`accountId` → `account_id`, `displayName` → `display_name`)
- [ ] Test: `handle_response` returns descriptive error for 401, 403, 404, 429 status codes [deferred — requires mock HTTP server]
- [ ] Test: Transition name matching is case-insensitive ("In Progress" matches "in progress") [deferred — requires mock HTTP server]
- [ ] Test: Transition matching returns available transitions in error when no match found [deferred — requires mock HTTP server]
- [x] Test: `create_pending_action` generates valid UUID, correct action_type, human-readable description
- [x] Test: `jira_tool_definitions()` returns 6 tools with correct names and required fields
- [x] Test: `format_issues_for_claude` produces readable output for 0, 1, and multiple issues
- [ ] Test: PendingAction expiry sweep marks actions older than 1 hour as Expired [deferred — expiry sweep deferred]

### Integration Test

- [ ] Create integration test (behind env var gate `NV_JIRA_INTEGRATION_TEST=1`): connect with real credentials, search issues with simple JQL, verify at least response parsing succeeds [deferred]
- [ ] Create integration test: full create flow — create issue on test project, verify `JiraCreatedIssue` returned with valid key, then transition to "In Progress", add comment, verify via `jira_get` [deferred]

### Verify

- [x] `cargo build` passes for all workspace members
- [x] `cargo test -p nv-daemon` — all unit tests pass
- [x] `cargo clippy` passes with no warnings
- [ ] Manual gate: "Create a P1 bug on OO" via Telegram → draft shown with inline keyboard → tap ✅ Create → issue exists in Jira with correct project, type, priority, title [user]
