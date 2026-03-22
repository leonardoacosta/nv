# harden-jira-integration

## Summary

Complete the 12 deferred tasks from the MVP `jira-integration` spec. Adds retry logic with exponential backoff, the remaining callback handlers (edit, cancel), pending action expiry sweep, callback routing in the agent loop, HTTP mock tests for error status codes, case-insensitive transition matching tests, and integration tests behind an env var gate.

## Motivation

The MVP Jira integration shipped with read tools, write tools, and the approve callback handler, but deferred reliability and completeness work. Without retries, transient 429/5xx errors surface directly to the user. Without the edit and cancel callback handlers, two of three inline keyboard buttons are dead. Without the expiry sweep, abandoned pending actions accumulate indefinitely. Without mock and integration tests, regressions go undetected.

This spec closes every deferred item so the Jira integration is production-grade.

## Design

### Retry Wrapper (`request_with_retry`)

Add `request_with_retry<F, Fut, T>(max_retries, f)` to `JiraClient`. Exponential backoff starting at 1s, doubling per attempt, max 3 retries. Only retries on:
- HTTP 429 (rate limit) -- respects `Retry-After` header if present
- HTTP 5xx (server error)
- Network/transport errors (reqwest connection failures)

All other errors (401, 403, 404, 400) propagate immediately. Existing methods (`jira_search`, `jira_get`, `jira_create`, `jira_transition`, `jira_assign`, `jira_comment`) are wrapped with `request_with_retry`.

```rust
async fn request_with_retry<F, Fut, T>(
    &self,
    max_retries: u32,
    f: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut backoff = Duration::from_secs(1);
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries && is_retryable(&e) => {
                tracing::warn!(
                    "Jira request failed (attempt {}/{}): {e}, retrying in {backoff:?}",
                    attempt + 1, max_retries
                );
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

fn is_retryable(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429")
        || msg.contains("rate limit")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("connection")
        || msg.contains("timed out")
}
```

### Callback Handler: `edit:{uuid}`

When user taps the Edit button on a pending action confirmation:
1. Load `PendingAction` by UUID from `~/.nv/state/pending-actions.json`
2. Reply with "What would you like to change?" referencing the original draft
3. The agent loop processes the user's next message as an edit instruction
4. Update the `PendingAction` payload with revised parameters
5. Re-send the draft summary with a fresh inline keyboard

The edit handler reuses `send_confirmation_keyboard` after mutating the stored payload. The agent loop must track that the next message from this chat is an edit response (store `editing_action_id: Option<String>` in agent loop state).

### Callback Handler: `cancel:{uuid}`

When user taps the Cancel button:
1. Load `PendingAction` by UUID
2. Set status to `Cancelled`
3. Write updated state back to `pending-actions.json`
4. Edit the original Telegram message to show cancellation notice (e.g., "Cancelled: Create Bug on OO")
5. Remove from active pending list

### Expiry Sweep

On each agent loop tick (or a dedicated periodic task), scan pending actions:
- Any `PendingAction` with status `Pending` and `created_at` older than 1 hour is marked `Expired`
- Edit the original Telegram message with expiry notice (e.g., "Expired: Create Bug on OO (no response after 1 hour)")
- Write updated state

Implementation: a `check_expired_actions(telegram, state_path)` async fn called from the agent loop's periodic maintenance. Uses `Utc::now() - action.created_at > Duration::hours(1)`.

### Callback Routing in Agent Loop

Wire the callback query dispatch in the agent loop. When an inbound message has content starting with `[callback]`, parse the callback data:

```rust
if content.starts_with("[callback] ") {
    let data = &content["[callback] ".len()..];
    if let Some(uuid) = data.strip_prefix("approve:") {
        handle_approve(uuid, &jira_client, &telegram, &state).await?;
    } else if let Some(uuid) = data.strip_prefix("edit:") {
        handle_edit(uuid, &telegram, &mut agent_state).await?;
    } else if let Some(uuid) = data.strip_prefix("cancel:") {
        handle_cancel(uuid, &telegram, &state).await?;
    }
}
```

The `approve` handler already exists; this wires `edit` and `cancel` alongside it and adds the Nexus callback prefixes as a passthrough (handled by spec-10).

### HTTP Mock Tests

Use `mockito` or `wiremock` crate to test `handle_response` and `request_with_retry` against simulated HTTP responses:
- 401 returns auth error message
- 403 returns permission denied message
- 404 returns not found message
- 429 triggers retry then succeeds (or exhausts retries)
- 500/502/503 trigger retry
- 200 returns parsed response

### Case-Insensitive Transition Matching Tests

Test that `jira_transition` matches transition names regardless of case. Mock the transitions endpoint to return `[{"id": "31", "name": "In Progress"}]` and verify that `"in progress"`, `"IN PROGRESS"`, and `"In Progress"` all resolve to ID `"31"`. Also test that a mismatch returns available transitions in the error message.

### Integration Tests (Env Var Gate)

Behind `NV_JIRA_INTEGRATION_TEST=1`:
1. Connect with real credentials, search issues with simple JQL, verify response parsing
2. Full create flow: create issue on test project, verify `JiraCreatedIssue` returned with valid key, transition to "In Progress", add comment, verify via `jira_get`

## Dependencies

- jira-integration (MVP spec -- already applied)

## Out of Scope

- Batch operations (bulk create/transition)
- Jira webhook ingestion (separate spec-8: `jira-webhooks`)
- Offline queue for pending actions (network-down scenario)
- Custom JQL builder UI

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- all new mock tests pass
- `cargo clippy` passes with no warnings
- Manual gate: via Telegram, trigger "Create a P1 bug on OO" -> tap Edit -> change title -> tap Approve -> issue created. Then trigger another -> tap Cancel -> cancelled notice shown. Wait 1 hour (or adjust TTL for testing) -> expired notice shown.
