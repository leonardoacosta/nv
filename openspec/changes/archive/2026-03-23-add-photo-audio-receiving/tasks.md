# Implementation Tasks

## DB Batch

(no database changes required)

## API Batch

- [x] [2.1] [P-1] Add `PhotoSize` struct to `telegram/types.rs`: `file_id: String`, `file_unique_id: String`, `width: i64`, `height: i64`, `file_size: Option<i64>` [owner:api-engineer]
- [x] [2.2] [P-1] Add `Audio` struct to `telegram/types.rs`: `file_id: String`, `file_unique_id: String`, `duration: i64`, `performer: Option<String>`, `title: Option<String>`, `mime_type: Option<String>`, `file_size: Option<i64>` [owner:api-engineer]
- [x] [2.3] [P-1] Add `photo: Option<Vec<PhotoSize>>`, `audio: Option<Audio>`, `caption: Option<String>` fields to `TgMessage` [owner:api-engineer]
- [x] [2.4] [P-1] Update `Update::to_inbound_message()` to detect photo messages: set metadata `"photo": true`, `"file_id"` (last/largest PhotoSize), `"caption"` if present; set content to caption or "User sent a photo." [owner:api-engineer]
- [x] [2.5] [P-1] Update `Update::to_inbound_message()` to detect audio messages: set metadata `"audio": true`, `"file_id"`, `"duration_secs"`, `"mime_type"`, `"title"` if present [owner:api-engineer]
- [x] [2.6] [P-1] Create `crates/nv-daemon/src/speech_to_text.rs`: ElevenLabs STT client — POST audio bytes to `/v1/speech-to-text`, parse JSON response for transcript text, 30s timeout [owner:api-engineer]
- [x] [2.7] [P-1] Add `handle_photo_message()` in `telegram/mod.rs`: download largest photo via `get_file()`/`download_file()`, save to `/tmp/nv-photo-{uuid}.jpg`, set `image_path` in metadata, use caption as content [owner:api-engineer]
- [x] [2.8] [P-1] Add `handle_audio_message()` in `telegram/mod.rs`: download audio file, call ElevenLabs STT for transcription, set content to caption + transcript, parallel to existing `transcribe_voice_message()` [owner:api-engineer]
- [x] [2.9] [P-1] Wire photo and audio handling in `run_poll_loop()`: after voice check, add photo check (call `handle_photo_message`) and audio check (call `handle_audio_message`) before `Trigger::Message` dispatch [owner:api-engineer]
- [x] [2.10] [P-1] Added `send_messages_cold_start_with_image()` accepting `Option<&str>` image path; adds `--attachment <path>` to `claude -p` args when present [owner:api-engineer]
- [x] [2.11] [P-1] Added `send_messages_with_image()` that bypasses persistent session when image present (persistent session doesn't support per-turn attachments); `send_messages()` delegates to it with `None` [owner:api-engineer]
- [x] [2.12] [P-2] Updated worker loop in `worker.rs` to extract `image_path` from trigger metadata and pass to `ClaudeClient::send_messages_with_image()` [owner:api-engineer]
- [x] [2.13] [P-2] Added temp file cleanup in `worker.rs` after Claude response: `std::fs::remove_file(path)` with warn-on-failure logging [owner:api-engineer]
- [x] [2.14] [P-2] Removed `#[allow(dead_code)]` from `get_file()` and `download_file()` in `telegram/client.rs` [owner:api-engineer]
- [x] [2.15] [P-2] Typing indicators sent via `send_chat_action("typing")` in `handle_photo_message()` and `handle_audio_message()` during download/processing [owner:api-engineer]
- [x] [2.16] [P-2] Updated existing `TgMessage` test fixtures in `telegram/types.rs` to include `photo: None`, `audio: None`, `caption: None` fields [owner:api-engineer]

## UI Batch

(no UI changes — Telegram-native media handling)

## E2E Batch

- [x] [4.1] Added unit tests for `PhotoSize` and `Audio` deserialization via struct field verification in `telegram/types.rs` [owner:test-writer]
- [x] [4.2] Added `photo_message_includes_metadata` and `photo_message_without_caption_uses_default_content` tests in `telegram/types.rs` [owner:test-writer]
- [x] [4.3] Added `audio_message_includes_metadata` and `audio_message_with_caption` tests in `telegram/types.rs` [owner:test-writer]
- [x] [4.4] Added unit tests in `speech_to_text.rs`: transcript extraction, missing field detection, empty transcript detection [owner:test-writer]
- [x] [4.5] `photo_message_without_caption_uses_default_content` test covers caption-less case [owner:test-writer]
- [x] [4.6] `audio_message_with_caption` test covers caption prepended to transcript content [owner:test-writer]
