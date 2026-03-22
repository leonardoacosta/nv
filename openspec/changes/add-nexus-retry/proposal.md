# Proposal: Add Nexus Retry Button

## Change ID
`add-nexus-retry`

## Summary

Add inline keyboard buttons on Nexus session error alerts: `[Retry]` and `[Create Bug]`. Retry
dispatches a new StartSession + SendCommand with the same spec/command that failed. Create Bug
uses the existing `create_bug_from_session_error` flow to file a beads issue.

## Context
- Extends: `crates/nv-daemon/src/orchestrator.rs` (error alert formatting), `crates/nv-daemon/src/callbacks.rs` (retry + create-bug handlers), `crates/nv-daemon/src/nexus/stream.rs` (session error event processing)
- Related: Existing `TriggerClass::NexusEvent` handling, `InlineKeyboard::confirm_action()` pattern, `SessionEvent` types, `PendingAction` confirmation flow
- Depends on: `mature-nexus-integration` (spec 20) — needs StartSession + SendCommand RPCs

## Motivation

When a Nexus session errors out, Nova sends a text alert to Telegram. Leo must then manually
re-run the command — often the same spec and project. An inline retry button reduces this to one
tap. For genuine bugs, the Create Bug button captures the error context into a beads issue without
requiring Leo to type anything.

## Requirements

### Req-1: Error Alert Keyboard

When a Nexus `SessionEvent` with error status arrives, the outbound alert message includes an
inline keyboard:

```
Session error on OO: /apply fix-chat-bugs
Error: Worker timeout after 300s

[🔄 Retry]  [🐛 Create Bug]
```

Keyboard layout: single row, two buttons.

### Req-2: Retry Callback Handler

When the user taps `[Retry]`:

1. Look up the original session's project and command from the error event metadata
2. Call `NexusClient.start_session(project, cwd, command)` — same as the original
3. Call `NexusClient.send_command(new_session_id, command)` if the original had a command
4. Edit the error message to: "Retrying... New session: {session_id}"
5. Remove the inline keyboard from the error message

Callback data format: `retry:{event_id}` where event_id maps to stored error metadata.

### Req-3: Create Bug Callback Handler

When the user taps `[Create Bug]`:

1. Look up the error event metadata (project, command, error message, session ID, timestamp)
2. Create a beads issue via `bd create` with the error context:
   - Title: "Session error: {command} on {project}"
   - Body: error message, session ID, timestamp
3. Edit the error message to: "Bug filed: {issue_id}"
4. Remove the inline keyboard from the error message

This reuses the existing `create_bug_from_session_error` pattern if it exists, or implements
a minimal version using `bd create`.

### Req-4: Error Metadata Storage

Store error event metadata in State (or a short-lived in-memory map) keyed by event_id so
the callback handlers can look up the original session details:

```rust
struct SessionErrorMeta {
    project: String,
    cwd: String,
    command: Option<String>,
    error_message: String,
    session_id: String,
    timestamp: DateTime<Utc>,
}
```

Entries expire after 24 hours (error alerts are only actionable shortly after they arrive).

## Scope
- **IN**: Inline keyboard on error alerts, retry callback handler, create-bug callback handler, error metadata storage with 24h expiry
- **OUT**: Automatic retry (always manual via button tap), retry count limits, error classification/triage, retry with modified parameters

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/orchestrator.rs` | Attach inline keyboard to NexusEvent error alerts, store error metadata |
| `crates/nv-daemon/src/callbacks.rs` | Add retry and create-bug callback handlers |
| `crates/nv-daemon/src/state.rs` | Add SessionErrorMeta storage with expiry |
| `crates/nv-core/src/types.rs` | Add InlineKeyboard::session_error(event_id) constructor |

## Risks
| Risk | Mitigation |
|------|-----------|
| Retry fails again (infinite retry) | No auto-retry — user must tap button each time. Error message updates with new failure. |
| Error metadata expires before user taps | 24h window is generous. Expired metadata → "Error details expired, please re-run manually." |
| Nexus agent offline during retry | Same error handling as StartSession — "Cannot reach Nexus agent" |
