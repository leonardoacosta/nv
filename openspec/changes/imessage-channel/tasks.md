# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [ ] [2.1] [P-1] Add IMessageConfig to nv-core config.rs: enabled (bool, default false), bluebubbles_url (String), bluebubbles_password (String), poll_interval_secs (u64, default 10) under [imessage] section [owner:api-engineer]
- [ ] [2.2] [P-1] Create crates/nv-daemon/src/imessage/mod.rs — BlueBubbles API types with serde: BbMessage (guid, text, date_created, handle, chat_guid, is_from_me), BbChat (guid, display_name, participants), BbHandle (address, service) [owner:api-engineer]
- [ ] [2.3] [P-1] Create BlueBubblesClient in imessage/mod.rs — reqwest HTTP client with base_url + password, get_messages(after, limit) via GET /api/v1/message, send_message(chat_guid, text) via POST /api/v1/message/text [owner:api-engineer]
- [ ] [2.4] [P-1] Create crates/nv-daemon/src/imessage/channel.rs — IMessageChannel implementing Channel trait: poll() fetches new messages since last_seen_timestamp, converts BbMessage to InboundMessage, send() calls BlueBubblesClient::send_message [owner:api-engineer]
- [ ] [2.5] [P-2] Add polling loop in imessage/channel.rs — tokio::spawn task that calls poll() on configurable interval, tracks last_seen timestamp, backs off on consecutive errors (double interval, cap 5 min) [owner:api-engineer]
- [ ] [2.6] [P-2] Wire iMessage channel into main.rs — if imessage.enabled, construct IMessageChannel, spawn polling task, connect to trigger mpsc channel [owner:api-engineer]
- [ ] [2.7] [P-2] Add mod imessage declaration in main.rs [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] Unit tests for BbMessage/BbChat/BbHandle serde deserialization from JSON fixtures [owner:test-writer]
- [ ] [3.4] Unit tests for BlueBubblesClient: mock HTTP responses, verify get_messages parsing and send_message request construction [owner:test-writer]
- [ ] [3.5] Unit test for polling loop: verify last_seen advances, verify backoff on error [owner:test-writer]
- [ ] [3.6] cargo test — all new + existing tests pass [owner:api-engineer]
