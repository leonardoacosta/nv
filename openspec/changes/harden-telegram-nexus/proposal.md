# harden-telegram-nexus

## Summary

Complete the 3 deferred tasks from the MVP `telegram-channel` spec (2 tasks) and `nexus-integration` spec (1 task). Adds a Telegram integration test against the real Bot API, a manual e2e gate for Telegram echo, and wires Nexus error callback routing (`nexus_err:view:` and `nexus_err:bug:` prefixes) into the Telegram handler.

## Motivation

The Telegram channel and Nexus integration both shipped functional but with testing and callback gaps deferred. The Telegram integration test ensures the bot token and API connectivity work against the real Telegram servers -- catching auth issues, network problems, and API changes before they hit production. The manual e2e gate validates the full message round-trip. The Nexus error callbacks complete the session error notification flow -- without them, the "View Error" and "Create Bug" inline buttons on Nexus error alerts are dead.

## Design

### Telegram Integration Test

Create an integration test behind `#[cfg(feature = "integration")]` and/or env var gate `NV_TELEGRAM_INTEGRATION_TEST=1`. The test:

1. Reads `TELEGRAM_BOT_TOKEN` and `NV_TEST_CHAT_ID` from environment
2. Creates a `TelegramClient` instance
3. Calls `get_me()` to verify token validity -- asserts `ok == true` and bot username is non-empty
4. Sends a test message to the configured chat: "Integration test: echo from nv-daemon"
5. Verifies the send response is successful (HTTP 200, `ok: true`)

```rust
#[cfg(feature = "integration")]
#[tokio::test]
async fn test_telegram_real_api() {
    let token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN required for integration test");
    let chat_id: i64 = std::env::var("NV_TEST_CHAT_ID")
        .expect("NV_TEST_CHAT_ID required for integration test")
        .parse()
        .expect("NV_TEST_CHAT_ID must be i64");

    let client = TelegramClient::new(&token);

    // Verify bot token
    let me = client.get_me().await.expect("get_me should succeed");
    assert!(!me.username.is_empty(), "Bot username should be non-empty");

    // Send echo message
    client.send_message(chat_id, "Integration test: echo from nv-daemon", None, None)
        .await
        .expect("send_message should succeed");
}
```

The `integration` feature is added to `crates/nv-daemon/Cargo.toml` under `[features]` with no extra deps. Tests run via `cargo test -p nv-daemon --features integration` when credentials are available.

### Telegram Manual E2E Gate

A documented manual verification step (not automated): set `TELEGRAM_BOT_TOKEN` and chat_id in config, run the daemon, send "hello" on Telegram, and verify the bot echoes back. This validates the full poll loop, message parsing, agent loop dispatch, and response sending.

### Nexus Error Callback Wiring

The Nexus integration (spec: `nexus-integration`) implemented `format_session_error` in `crates/nv-daemon/src/nexus/notify.rs` which sends error alerts with inline buttons:
- "View Error" -- callback data: `nexus_err:view:{session_id}`
- "Create Bug" -- callback data: `nexus_err:bug:{session_id}`

These callbacks arrive as `[callback] nexus_err:view:{session_id}` or `[callback] nexus_err:bug:{session_id}` in the agent loop. The wiring:

```rust
// In callback routing (agent_loop.rs):
if let Some(rest) = data.strip_prefix("nexus_err:") {
    if let Some(session_id) = rest.strip_prefix("view:") {
        // Fetch session error details via NexusClient::query_session(session_id)
        // Send full error text as a Telegram reply message
        let session = nexus_client.query_session(session_id).await?;
        telegram.send_message(chat_id, &format_session_error_detail(&session), None, None).await?;
    } else if let Some(session_id) = rest.strip_prefix("bug:") {
        // Push a Jira create trigger with pre-filled context:
        // project: extracted from session, title: "Session error: {project}",
        // description: error message + session context
        let session = nexus_client.query_session(session_id).await?;
        let action = create_bug_from_session_error(&session, &jira_client);
        save_pending_action(&action).await?;
        send_confirmation_keyboard(&telegram, &action).await?;
    }
}
```

The `view` handler queries the session and sends the full error as a follow-up Telegram message. The `bug` handler creates a `PendingAction` of type `JiraActionType::Create` with the error context pre-filled in the description, then goes through the standard Jira confirmation flow.

## Dependencies

- telegram-channel (MVP spec -- already applied)
- nexus-integration (MVP spec -- already applied)
- jira-integration (for `bug` callback -- Create Bug uses Jira pending action flow)

## Out of Scope

- Automated e2e test (would require a second bot or test account polling for responses)
- Telegram webhook mode (staying with long-poll for simplicity)
- Nexus session control callbacks (start/stop session -- deferred to future spec)
- Retry button on session errors (deferred)

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- existing tests pass
- `cargo test -p nv-daemon --features integration` -- Telegram integration test passes (requires real credentials)
- `cargo clippy` passes with no warnings
- Manual gate: run daemon, send "hello" on Telegram, bot echoes back
- Manual gate: trigger a Nexus session error notification, tap "View Error" -- full error displayed, tap "Create Bug" -- Jira draft shown with pre-filled context
