# Implementation Tasks

<!-- beads:epic:TBD -->

## Error Metadata Storage

- [ ] [1.1] [P-1] Add SessionErrorMeta struct to state.rs — project, cwd, command, error_message, session_id, timestamp fields [owner:api-engineer]
- [ ] [1.2] [P-1] Add session_errors HashMap<String, SessionErrorMeta> to State — keyed by event_id (UUID) [owner:api-engineer]
- [ ] [1.3] [P-2] Add store_session_error(meta) and get_session_error(event_id) methods to State — with 24h expiry check on get [owner:api-engineer]
- [ ] [1.4] [P-2] Add prune_expired_errors() method to State — called periodically from orchestrator expiry check loop [owner:api-engineer]

## Error Alert Keyboard

- [ ] [2.1] [P-1] Add InlineKeyboard::session_error(event_id) constructor to nv-core/src/types.rs — returns [Retry] [Create Bug] row [owner:api-engineer]
- [ ] [2.2] [P-1] Update NexusEvent error handling in orchestrator.rs — store SessionErrorMeta, attach session_error keyboard to outbound alert [owner:api-engineer]

## Callback Handlers

- [ ] [3.1] [P-1] Add handle_retry(event_id, nexus_client, telegram, chat_id, state) to callbacks.rs — look up error meta, call start_session + send_command, edit original message [owner:api-engineer]
- [ ] [3.2] [P-1] Add handle_create_bug(event_id, telegram, chat_id, state) to callbacks.rs — look up error meta, run bd create with error context, edit original message [owner:api-engineer]
- [ ] [3.3] [P-2] Wire retry:{event_id} and bug:{event_id} callback patterns in orchestrator callback router [owner:api-engineer]

## Integration

- [ ] [4.1] [P-2] Handle expired error metadata gracefully — return "Error details expired, please re-run manually" and remove keyboard [owner:api-engineer]

## Verify

- [ ] [5.1] cargo build passes [owner:api-engineer]
- [ ] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [5.3] cargo test — new tests for SessionErrorMeta storage/expiry, keyboard construction, callback routing [owner:api-engineer]
