# Tasks: add-obligation-detection

## Dependencies

- `add-obligation-store` (must have store to persist detected obligations)

## Tasks

### Detector Module (already complete)

- [x] [1.1] Create `crates/nv-daemon/src/obligation_detector.rs` with `DetectedObligation` struct (detected_action, priority, owner, owner_reason, project_code) [owner:api-engineer]
- [x] [1.2] Define `ClassifierJson` internal deserialization struct matching the Claude JSON response shape (is_obligation, detected_action, priority, owner, owner_reason, project_code) [owner:api-engineer]
- [x] [1.3] Define `CliResponse` internal struct for the outer `claude -p --output-format json` envelope (result, is_error) [owner:api-engineer]
- [x] [1.4] Write `CLASSIFIER_SYSTEM_PROMPT` const -- obligation detection rules, owner classification (nova/leo), priority scale 0-4, JSON-only response format [owner:api-engineer]
- [x] [1.5] Implement `detect_obligation(message_content, channel) -> Result<Option<DetectedObligation>>` -- spawn `claude -p` subprocess, write prompt to stdin, parse JSON response, validate fields, clamp priority 0-4, default unknown owner to "nova" [owner:api-engineer]
- [x] [1.6] Add HOME/REAL_HOME resolution -- check REAL_HOME first (systemd), fallback to HOME, hard error if neither set [owner:api-engineer]
- [x] [1.7] Add 30-second subprocess timeout via `tokio::time::timeout` [owner:api-engineer]

### Orchestrator Integration (already complete)

- [x] [2.1] Wire obligation detection in `orchestrator.rs` `process_trigger_batch()` -- fire-and-forget `tokio::spawn` on inbound `Trigger::Message` events only [owner:api-engineer]
- [x] [2.2] Create message excerpt (first 200 chars) for `source_message` field in `NewObligation` [owner:api-engineer]
- [x] [2.3] On `Some(detected)`: construct `NewObligation` with `Uuid::new_v4()`, map owner string to `ObligationOwner` enum, call `ObligationStore::create()` via SharedDeps mutex [owner:api-engineer]
- [x] [2.4] On P0-P1 detection: send Telegram notification with obligation card and inline keyboard [owner:api-engineer]
- [x] [2.5] Graceful error handling: detection failure logs at debug level and continues (non-fatal) [owner:api-engineer]

### Existing Tests (already complete)

- [x] [3.1] Test `ClassifierJson` deserializes obligation with all fields (is_obligation, detected_action, priority, owner, owner_reason, project_code) [owner:api-engineer]
- [x] [3.2] Test `ClassifierJson` deserializes non-obligation (`{"is_obligation": false}`) with defaults [owner:api-engineer]
- [x] [3.3] Test `CliResponse` deserializes outer envelope (result, is_error) [owner:api-engineer]
- [x] [3.4] Test priority clamping -- values outside 0-4 range are clamped correctly [owner:api-engineer]
- [x] [3.5] Test `detect_obligation` returns Err when both HOME and REAL_HOME are unset [owner:api-engineer]

### Remaining Tests

- [ ] [4.1] Unit test: `ClassifierJson` with missing optional fields -- deserialize JSON with no `project_code` and no `owner_reason`, verify defaults to None/empty [owner:api-engineer]
- [ ] [4.2] Unit test: unknown owner value handling -- simulate classifier returning `owner: "unknown_entity"`, verify detection logic would default to "nova" (test the match arm in detect_obligation) [owner:api-engineer]
- [ ] [4.3] Unit test: empty detected_action with is_obligation=true -- verify the validation check returns None (not a valid obligation without an action description) [owner:api-engineer]

### Verify

- [ ] [5.1] `cargo build` passes for all workspace members [owner:api-engineer]
- [ ] [5.2] `cargo test -p nv-daemon` -- all obligation_detector tests pass [owner:api-engineer]
- [ ] [5.3] `cargo clippy -- -D warnings` passes [owner:api-engineer]
