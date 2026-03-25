# Implementation Tasks: fix-cold-start-memory

<!-- beads:epic:TBD -->

## DB Batch
(no database schema changes)

## API Batch

- [ ] [2.1] [P-1] Inspect MessageStore in messages.rs — confirm whether `get_recent_outbound` or equivalent already exists [owner:api-engineer]
- [ ] [2.2] [P-1] Add `get_recent_outbound(limit: usize) -> Vec<OutboundMessage>` to MessageStore in messages.rs if missing — query messages table ordered by timestamp DESC, limit N [owner:api-engineer]
- [ ] [2.3] [P-1] In worker.rs: before building the cold-start conversation payload, query `message_store.get_recent_outbound(10)` [owner:api-engineer]
- [ ] [2.4] [P-1] In worker.rs: format the outbound slice as a `String` — each line "[HH:MM] Nova: {preview}" (preview capped at 200 chars), empty string if slice is empty [owner:api-engineer]
- [ ] [2.5] [P-1] Pass the formatted context string to `send_messages_cold_start_with_image()` as a new `recent_context: Option<&str>` parameter [owner:api-engineer]
- [ ] [2.6] [P-1] In claude.rs `send_messages_cold_start_with_image()`: accept `recent_context: Option<&str>` parameter [owner:api-engineer]
- [ ] [2.7] [P-1] In claude.rs: when `recent_context` is `Some` and non-empty, prepend "Your recent messages to Leo:\n{context}\n\n" section to the system prompt string before the main instructions [owner:api-engineer]
- [ ] [2.8] [P-2] Ensure digest path in worker.rs also passes recent context — digests call cold-start and currently bypass ConversationStore entirely [owner:api-engineer]

## UI Batch
(no UI changes)

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit test: `get_recent_outbound(5)` returns at most 5 rows ordered newest-first [owner:api-engineer]
- [ ] [3.4] Unit test: `get_recent_outbound(10)` on empty store returns empty vec (no panic) [owner:api-engineer]
- [ ] [3.5] Unit test: format helper produces "[HH:MM] Nova: ..." lines, truncates preview at 200 chars [owner:api-engineer]
- [ ] [3.6] Unit test: `send_messages_cold_start_with_image()` with `Some(context)` includes "Your recent messages to Leo:" in system prompt [owner:api-engineer]
- [ ] [3.7] Unit test: `send_messages_cold_start_with_image()` with `None` or empty context omits the section entirely [owner:api-engineer]
- [ ] [3.8] Existing tests pass [owner:api-engineer]

## E2E

- [ ] [4.1] Send message to Nova, wait for response, send follow-up referencing the response — Nova should understand the reference without needing `get_recent_messages` tool call to recover context [owner:api-engineer]
