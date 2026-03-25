# Implementation Tasks

<!-- beads:epic:nv-lsmv -->

## API Batch

- [ ] [2.1] Add `read_stream_response_streaming<R, F>` to `claude.rs` — identical loop to `read_stream_response_from_lines` but calls `on_text_delta(&str)` on each `assistant/text` event before accumulating; expose as `pub(crate)` [owner:api-engineer]
- [ ] [2.2] Wire `read_stream_response_streaming` into `PersistentSession::send_turn` — pass through the callback so the caller (worker) can attach the edit closure; update `read_stream_response` to accept and forward the callback [owner:api-engineer]
- [ ] [2.3] In `Worker::run`, after extracting `tg_chat_id` and before calling Claude: if trigger is a Telegram message and `tg_client` is `Some`, call `tg_client.send_thinking(chat_id)` and store returned `placeholder_msg_id: Option<i64>` [owner:api-engineer]
- [ ] [2.4] Remove the `send_chat_action("typing")` call at turn start for Telegram triggers when a placeholder is sent (keep it for CLI/cron triggers where no placeholder is used) [owner:api-engineer]
- [ ] [2.5] Build the streaming edit closure in `Worker::run` — captures `tg_client`, `placeholder_msg_id`, `stream_buffer: String`, `last_edit_at: Instant`, `chars_since_last_edit: usize`; fires `tokio::spawn(edit_message_text(...))` when interval or delta threshold is met; reset counters after each fire [owner:api-engineer]
- [ ] [2.6] After tool loop completes: if `placeholder_msg_id` is `Some` and `response_text` is non-empty, call `tg_client.edit_message(chat_id, placeholder_msg_id, &response_text, keyboard.as_ref())` instead of `channel.send_message` [owner:api-engineer]
- [ ] [2.7] After tool loop completes: if `placeholder_msg_id` is `Some` and `response_text` is empty (tool-only turn), call `tg_client.delete_message(chat_id, placeholder_msg_id)` [owner:api-engineer]
- [ ] [2.8] Preserve existing `channel.send_message` delivery path for cold-start turns (no `placeholder_msg_id`) — no change to that branch [owner:api-engineer]
- [ ] [2.9] Ensure keyboard is only attached on the final edit (Req-7) — intermediate streaming edits use `edit_message_text` (no keyboard); final edit uses `edit_message` with optional keyboard [owner:api-engineer]

## Verify

- [ ] [3.1] `cargo build -p nv-daemon` passes [owner:api-engineer]
- [ ] [3.2] `cargo clippy -p nv-daemon -- -D warnings` passes [owner:api-engineer]
- [ ] [3.3] Unit test: `read_stream_response_streaming` calls `on_text_delta` once per `assistant/text` event with correct delta text [owner:api-engineer]
- [ ] [3.4] Unit test: `read_stream_response_streaming` returns the same `ApiResponse` as `read_stream_response_from_lines` for identical input [owner:api-engineer]
- [ ] [3.5] Unit test: streaming edit closure fires when `STREAMING_EDIT_INTERVAL_MS` elapses (mock time) [deferred — requires async time mock]
- [ ] [3.6] Unit test: streaming edit closure fires when accumulated delta >= `STREAMING_EDIT_MIN_DELTA_CHARS` regardless of elapsed time [owner:api-engineer]
- [ ] [3.7] Unit test: tool-only turn (empty `response_text`) calls `delete_message` not `edit_message` [deferred — requires integration harness]
- [ ] [3.8] Existing unit tests pass (no regression to `read_stream_response_from_lines`, `extract_text`, markdown_to_html) [owner:api-engineer]
