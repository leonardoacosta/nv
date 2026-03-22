# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add EmailConfig to nv-core config.rs: enabled (bool, default false), poll_interval_secs (u64, default 60), folder_ids (Vec<String>, default ["Inbox"]), sender_filter (Vec<String>, default []), subject_filter (Vec<String>, default []) under [email] section [owner:api-engineer]
- [x] [2.2] [P-1] Create crates/nv-daemon/src/email/mod.rs — email types with serde: GraphMailMessage (id, subject, body_preview, body content/content_type, from address, received_date_time, conversation_id), GraphMailFolder (id, display_name) [owner:api-engineer]
- [x] [2.3] [P-1] Add mail methods to shared MsGraphClient (or email/mod.rs wrapping it): get_messages(folder_id, after, top) via GET /me/mailFolders/{folder}/messages with OData filter, send_mail(to, subject, body) via POST /me/sendMail, reply_to_message(message_id, body) via POST /me/messages/{id}/reply [owner:api-engineer]
- [x] [2.4] [P-1] Create crates/nv-daemon/src/email/html.rs — html_to_text() function: strip HTML tags, decode common entities (&amp; &lt; &gt; &nbsp; &quot;), preserve paragraph/line breaks, return plain text String [owner:api-engineer]
- [x] [2.5] [P-1] Create crates/nv-daemon/src/email/channel.rs — EmailChannel implementing Channel trait: poll() fetches new messages from configured folders, applies sender_filter and subject_filter, converts to InboundMessage with html_to_text body, send() creates PendingAction for reply confirmation [owner:api-engineer]
- [x] [2.6] [P-2] Add polling loop in email/channel.rs — tokio::spawn task polling on configurable interval, tracks last_seen receivedDateTime per folder, backs off on consecutive errors [owner:api-engineer]
- [x] [2.7] [P-2] Wire PendingAction reply flow — on Telegram callback approval, call MsGraphClient::reply_to_message with original message_id for proper threading [owner:api-engineer]
- [x] [2.8] [P-2] Wire email channel into main.rs — if email.enabled, construct EmailChannel with shared MsGraphClient, spawn polling task, connect to trigger mpsc channel [owner:api-engineer]
- [x] [2.9] [P-2] Add mod email declaration in main.rs [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit tests for GraphMailMessage serde deserialization from JSON fixtures [owner:test-writer]
- [x] [3.4] Unit tests for html_to_text: basic HTML stripping, entity decoding, nested tags, empty input, plain text passthrough [owner:test-writer]
- [x] [3.5] Unit tests for sender_filter and subject_filter matching logic [owner:test-writer]
- [x] [3.6] Unit tests for MsGraphMailClient: mock HTTP responses, verify get_messages OData filter construction, send_mail request body, reply threading [owner:test-writer]
- [x] [3.7] Unit test for polling loop: verify last_seen advances per folder, verify backoff on error [owner:test-writer]
- [x] [3.8] cargo test — all new + existing tests pass [owner:api-engineer]
