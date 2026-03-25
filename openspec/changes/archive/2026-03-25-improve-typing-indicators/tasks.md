# Implementation Tasks

<!-- beads:epic:TBD -->

## API Batch

- [x] [2.1] [P-1] Add `ThrottleState` struct to `client.rs` — fields: `last_sent: HashMap<i64, Instant>`, `backoff_until: HashMap<i64, Instant>` [owner:api-engineer]
- [x] [2.2] [P-1] Wrap `ThrottleState` in `Arc<Mutex<ThrottleState>>` as a field on `TelegramClient`; update `new()` and `Clone` derive accordingly [owner:api-engineer]
- [x] [2.3] [P-1] Rewrite `send_chat_action` to check throttle before sending: suppress if `last_sent[chat_id]` is within 5s or `backoff_until[chat_id]` is in the future; update `last_sent` on successful send [owner:api-engineer]
- [x] [2.4] [P-1] In `send_chat_action`, parse 429 responses — read `parameters.retry_after` from the Telegram error JSON; set `backoff_until[chat_id] = Instant::now() + Duration::from_secs(retry_after)`, defaulting to 30s if field absent [owner:api-engineer]
- [x] [2.5] [P-2] Add `worker_chat_id: HashMap<Uuid, i64>` field to `Orchestrator` struct; initialize as empty in `new()` [owner:api-engineer]
- [x] [2.6] [P-1] In `handle_worker_event`, populate `worker_chat_id` on `WorkerEvent::StageStarted` from the worker task's `telegram_chat_id`; remove entry on `WorkerEvent::Complete` and `WorkerEvent::Error` [owner:api-engineer]
- [x] [2.7] [P-1] In `handle_worker_event`, on `WorkerEvent::ToolCalled`: look up `worker_chat_id[worker_id]`, then call `tg_channel.client.send_chat_action(chat_id, "typing").await` if a chat_id is found [owner:api-engineer]
- [x] [2.8] [P-2] In `worker.rs`, remove the dead `typing_cancel` watch channel declaration (lines ~479-480: `let typing_cancel = ...` and `let typing_tx = typing_cancel.0`) — the variable is written once and never used [owner:api-engineer]

## Verify

- [x] [3.1] cargo build passes [owner:api-engineer]
- [x] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [3.3] Unit test: `send_chat_action` is suppressed when called twice within 5s for the same chat_id [owner:api-engineer]
- [x] [3.4] Unit test: `send_chat_action` is not suppressed when called for two different chat_ids within 5s [owner:api-engineer]
- [x] [3.5] Unit test: 429 response with `retry_after: 10` sets backoff_until ~10s in the future and suppresses subsequent calls [owner:api-engineer]
- [x] [3.6] Unit test: 429 response with missing `retry_after` defaults to 30s backoff [owner:api-engineer]
- [x] [3.7] Existing tests pass [owner:api-engineer]
