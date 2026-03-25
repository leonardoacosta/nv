# Implementation Tasks

<!-- beads:epic:nv-uvhs -->

## DB Batch

(no database changes required)

## API Batch

- [ ] [2.1] [P-1] Add `is_voice_trigger: bool` field to `WorkerTask` in `worker.rs` (default `false`); update all `WorkerTask { ... }` construction sites to include the new field [owner:api-engineer] [beads:nv-dyxk]
- [ ] [2.2] [P-1] In `orchestrator.rs` `process_trigger_batch`: detect voice origin by iterating over `triggers` with `Trigger::Message(msg) => msg.metadata.get("voice").and_then(|v| v.as_bool()).unwrap_or(false)` using `Iterator::any`; assign result to `WorkerTask.is_voice_trigger` [owner:api-engineer] [beads:nv-lrwf]
- [ ] [2.3] [P-1] In `worker.rs` voice delivery block (lines ~1556–1590): add `task.is_voice_trigger` and `tool_names.is_empty()` as required conditions before spawning TTS task; capture `is_voice_trigger` from task before `run_worker` consumes it [owner:api-engineer] [beads:nv-w18d]
- [ ] [2.4] [P-2] Add `caption: Option<&str>` parameter to `TelegramClient::send_voice` in `channels/telegram/client.rs`; append `caption` and `parse_mode = "HTML"` fields to the multipart form when `Some`; truncate caption to 1024 chars with trailing `…` if needed [owner:api-engineer] [beads:nv-43xe]
- [ ] [2.5] [P-2] Update the `send_voice` call site in `worker.rs` to pass `Some(&response_text)` as caption [owner:api-engineer] [beads:nv-a4pp]

## UI Batch

(no UI changes — Telegram-native voice bubbles)

## E2E Batch

- [ ] [4.1] Add unit test for `WorkerTask.is_voice_trigger` default: verify field is `false` when constructed without voice metadata [owner:test-writer] [beads:nv-0r2s]
- [ ] [4.2] Add unit test for orchestrator voice-origin detection: mock a trigger batch containing a `Trigger::Message` with `metadata["voice"] = true` and assert `is_voice_trigger == true`; add complementary test with text-only trigger asserting `false` [owner:test-writer] [beads:nv-52xa]
- [ ] [4.3] Add unit test for TTS gate conditions in `worker.rs`: verify voice delivery is skipped when `is_voice_trigger = false` (text message), when `tool_names` is non-empty, and when `response_text` exceeds `voice_max_chars` [owner:test-writer] [beads:nv-xuq2]
- [ ] [4.4] Add unit test for `send_voice` caption truncation: verify that a string of 1025 chars is truncated to 1024 chars ending with `…` [owner:test-writer] [beads:nv-pnst]
