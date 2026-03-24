# Proposal: Add Obligation Detection

## Change ID
`add-obligation-detection`

## Summary

After inbound messages arrive, classify them with Claude to determine whether they contain
an action request or commitment. Identify the responsible party (Nova or Leo), the associated
project, and the priority. Store detected obligations via `ObligationStore`. Classify owner as
`NOVA_CAN_HANDLE` or `LEO_MUST_HANDLE` based on whether the action requires human judgment.

## Context
- Depends on: `add-obligation-store`
- Files: `crates/nv-daemon/src/obligation_detector.rs` (new), `crates/nv-daemon/src/orchestrator.rs`
  (integration), `crates/nv-daemon/src/worker.rs` (SharedDeps access)
- Scope-lock reference: Phase 3 "Proactive behavior" -- obligation detection

## Motivation

Messages arrive from Telegram, Discord, and other channels throughout the day. Many contain
implicit or explicit commitments ("deploy the auth service by Friday", "please check the CI
failure", "we need to update the API docs"). Without automated detection, these obligations
are forgotten -- Nova's "amnesia problem" extends to commitments, not just conversation context.

Detection enables the obligation engine pipeline: message -> classify -> store -> notify.

## Design

### Detector Module (obligation_detector.rs)

A lightweight classifier that spawns `claude -p --output-format json --no-session-persistence`
with a focused system prompt. No tools, no persistent session -- single-turn classification only.

**System prompt instructs Claude to:**
- Identify obligations: explicit promises, action items, deadlines, blocked/waiting tasks
- Reject non-obligations: status updates, answered questions, acknowledgements, FYI messages
- Return structured JSON with `is_obligation`, `detected_action`, `priority` (0-4), `owner`
  ("nova" or "leo"), `owner_reason`, and `project_code`

**Owner classification rules:**
- `nova` = Nova can handle autonomously (send message, run command, look something up)
- `leo` = Requires Leo's judgment or physical presence

### Detection Types

- `DetectedObligation` struct: `detected_action`, `priority`, `owner`, `owner_reason`, `project_code`
- `ClassifierJson` (internal): deserialization target for Claude's JSON response
- `CliResponse` (internal): outer envelope from `claude -p --output-format json`

### Orchestrator Integration

Detection runs as a fire-and-forget `tokio::spawn` in `process_trigger_batch()`, triggered only
on inbound `Trigger::Message` events:

1. Extract message content and channel from the trigger
2. Create an excerpt (first 200 chars) for source_message storage
3. Spawn async task that calls `detect_obligation(content, channel)`
4. On `Some(detected)`: create `NewObligation` with UUID, store via `ObligationStore`
5. On P0-P1 detection: send Telegram notification with card + inline keyboard (delegated to
   `add-obligation-telegram-ux` spec)
6. On `None` or `Err`: log and continue (detection failure is non-fatal)

### Environment Handling

The detector resolves HOME from `REAL_HOME` (falling back to `HOME`) because the daemon runs
under systemd where HOME may not be the user's home directory. Returns a hard error if neither
is set.

### Timeout

30-second timeout on the Claude subprocess prevents hung processes from blocking detection
indefinitely.

## Current State

This work is **already implemented**:
- `obligation_detector.rs` exists with full `detect_obligation()` async function
- System prompt covers obligation vs non-obligation classification
- Owner classification (nova/leo) with reason
- Priority 0-4 with clamping
- JSON parsing with graceful error handling (returns None, not Err)
- Orchestrator integration in `process_trigger_batch()` with fire-and-forget spawn
- HOME/REAL_HOME resolution for systemd environments
- 30-second subprocess timeout
- Unit tests for JSON deserialization, priority clamping, HOME-unset error

## Remaining Work

- Unit test: verify `ClassifierJson` handles missing optional fields gracefully (no project_code,
  no owner_reason)
- Unit test: verify unknown owner values default to "nova"
- Unit test: verify empty `detected_action` with `is_obligation=true` returns None
- Verify `cargo build` gate passes

## Dependencies

- `add-obligation-store` (must have store to persist detected obligations)

## Out of Scope

- Telegram notification formatting (separate spec: `add-obligation-telegram-ux`)
- Batch detection (processing multiple messages in one Claude call)
- Detection accuracy tuning or prompt iteration
- Cron-based re-classification of existing messages

## Verification

- `cargo build` passes for all workspace members
- `cargo test -p nv-daemon` -- all obligation_detector tests pass
- `cargo clippy -- -D warnings` passes
