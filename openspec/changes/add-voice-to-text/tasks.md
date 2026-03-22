# Implementation Tasks

<!-- beads:epic:TBD -->

## Telegram Voice Types

- [x] [1.1] [P-1] Add Voice struct to telegram/types.rs — file_id, file_unique_id, duration, mime_type, file_size fields [owner:api-engineer]
- [x] [1.2] [P-1] Add voice: Option<Voice> field to the Message struct in telegram/types.rs [owner:api-engineer]
- [x] [1.3] [P-1] Update to_inbound_message() to detect voice messages and flag them in metadata as {"voice": true, "file_id": "...", "duration_secs": N} [owner:api-engineer]

## Telegram File Download

- [x] [2.1] [P-1] Add get_file(file_id) method to TelegramClient — calls getFile API, returns file_path [owner:api-engineer]
- [x] [2.2] [P-1] Add download_file(file_path) method to TelegramClient — downloads from Telegram file server, returns Vec<u8> [owner:api-engineer]

## Deepgram Transcription

- [x] [3.1] [P-1] Add transcribe_voice(audio_bytes, mime_type) function — POST to Deepgram API, parse transcript from response JSON [owner:api-engineer]
- [x] [3.2] [P-2] Handle Deepgram error cases — empty transcript ("No speech detected"), API error ("Transcription failed"), missing API key ("Voice transcription not configured") [owner:api-engineer]
- [x] [3.3] [P-2] Add optional deepgram_model field to AgentConfig in nv-core/src/config.rs (default: "nova-2") [owner:api-engineer]

## Voice Message Pipeline

- [x] [4.1] [P-1] Update poll_messages in telegram/mod.rs — detect voice messages (voice field present), call get_file + download_file + transcribe_voice [owner:api-engineer]
- [x] [4.2] [P-2] Create InboundMessage from transcribed text — set content to transcript, preserve sender, add voice metadata [owner:api-engineer]
- [x] [4.3] [P-2] Send typing indicator while transcribing — call sendChatAction("typing") before Deepgram call [owner:api-engineer]
- [x] [4.4] [P-2] On transcription failure, send error reply to Telegram chat — "Could not transcribe voice message" with reply_to original message [owner:api-engineer]

## Verify

- [x] [5.1] cargo build passes [owner:api-engineer]
- [x] [5.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [x] [5.3] cargo test — new tests for Voice struct parsing, get_file response parsing, Deepgram response parsing, error handling paths [owner:api-engineer]
