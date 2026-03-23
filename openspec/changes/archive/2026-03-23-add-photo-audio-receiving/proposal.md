# Proposal: Add Photo and Audio Receiving

## Change ID
`add-photo-audio-receiving`

## Summary

Enable Nova to receive photos (with Claude vision) and audio files (MP3, WAV via ElevenLabs STT), extending the Telegram media pipeline beyond the current text + voice-note support.

## Context
- Extends: `crates/nv-daemon/src/telegram/types.rs` (TgMessage struct), `crates/nv-daemon/src/telegram/mod.rs` (poll loop media handling), `crates/nv-daemon/src/claude.rs` (image attachment support)
- Related: Existing voice pipeline (Deepgram STT for OGG voice notes), TTS outbound (ElevenLabs), `get_file()`/`download_file()` in `telegram/client.rs` (generic, already work for any file type)
- Depends on: none

## Motivation

Photos sent by users currently result in "Got an empty message" because `TgMessage` has no `photo` field — the message arrives with `text: None` and `voice: None`, producing an empty `content` string. Audio files (MP3, WAV sent as documents or via Telegram's audio type) are similarly dropped.

Photos are the most natural way to share context — screenshots, error messages, diagrams, receipts. Claude has vision capability and can analyze images directly. Audio files (podcasts, recordings, meeting audio) need transcription before processing, which ElevenLabs Speech-to-Text handles well and is already the preferred audio provider for TTS output.

## Requirements

### Req-1: Photo Message Detection

Add `photo: Option<Vec<PhotoSize>>` and `caption: Option<String>` to `TgMessage`. Telegram sends photos as an array of `PhotoSize` objects (different resolutions of the same image). Each has `file_id`, `file_unique_id`, `width`, `height`, `file_size`.

Update `to_inbound_message()` to detect photo messages and include `"photo": true`, `"file_id"` (largest resolution), and `"caption"` in metadata.

### Req-2: Photo Download and Claude Vision

When a photo message is detected in the poll loop:

1. Select the largest `PhotoSize` by file_size (or last in array — Telegram orders smallest-first)
2. Download via existing `get_file()` + `download_file()`
3. Save to a temp file (`/tmp/nv-photo-{uuid}.jpg`)
4. Set `content` to the caption if present, otherwise "User sent a photo."
5. Include the temp file path in metadata as `"image_path"` for downstream processing
6. Clean up the temp file after the Claude turn completes

The agent loop and Claude client must be extended to pass image attachments to the Claude CLI. The cold-start path adds `--attachment <path>` to the `claude -p` invocation. The persistent session path includes the image path in the stream-json input.

### Req-3: Audio File Detection

Add `audio: Option<Audio>` struct to `TgMessage` for Telegram's `Audio` type (MP3, WAV, etc. sent via the audio player — distinct from voice notes which use the `Voice` type).

The `Audio` struct includes: `file_id`, `file_unique_id`, `duration`, `performer`, `title`, `mime_type`, `file_size`.

Update `to_inbound_message()` to detect audio messages and include `"audio": true`, `"file_id"`, `"duration_secs"`, `"mime_type"`, and `"title"` in metadata.

### Req-4: Audio Transcription via ElevenLabs STT

When an audio file message is detected in the poll loop:

1. Download via existing `get_file()` + `download_file()`
2. Transcode to a Deepgram/ElevenLabs-compatible format if needed (ffmpeg)
3. POST to ElevenLabs Speech-to-Text API (`/v1/speech-to-text`)
4. Extract transcript text
5. Set `content` to `"{caption}\n\n[Transcription]: {transcript}"` if caption present, otherwise just the transcript
6. Include `"audio": true` and transcription metadata in the InboundMessage

A new `transcribe_audio_elevenlabs()` function in a `speech_to_text.rs` module (or extend `voice_input.rs`) handles the ElevenLabs STT HTTP call. The existing Deepgram pipeline for voice notes remains unchanged.

### Req-5: Caption Support

Both photo and audio messages in Telegram can carry a `caption` field (up to 1024 chars). When present:
- Photos: use caption as the `content` text alongside the image attachment
- Audio: prepend caption to the transcription result

The `caption` field is already added to `TgMessage` in Req-1; this requirement covers its use in content assembly.

### Req-6: Temp File Cleanup

All temp files created during photo processing (`/tmp/nv-photo-*.jpg`) must be cleaned up after the Claude turn completes. Use a guard/cleanup pattern — either `Drop` impl or explicit cleanup in the agent loop's post-response path. If the daemon crashes, `/tmp` is cleaned on reboot.

## Scope
- **IN**: Photo receiving + Claude vision, audio file receiving + ElevenLabs STT, caption support, temp file management, metadata enrichment, `--attachment` support in Claude CLI invocation
- **OUT**: Photo OCR (use Claude vision instead), video messages, document/file messages (PDF, etc.), outbound photo/audio sending, audio file storage, Whisper fallback, streaming transcription, Deepgram for audio files (keep Deepgram for voice notes only)

## Impact
| Area | Change |
|------|--------|
| `telegram/types.rs` | Add `PhotoSize`, `Audio` structs; add `photo`, `audio`, `caption` fields to `TgMessage`; update `to_inbound_message()` metadata |
| `telegram/mod.rs` | Add `handle_photo_message()` and `handle_audio_message()` in poll loop, parallel to existing `transcribe_voice_message()` |
| `telegram/client.rs` | Remove `#[allow(dead_code)]` from `get_file()`/`download_file()` (now actively used for photos/audio) |
| `speech_to_text.rs` (new) | ElevenLabs STT client: POST audio bytes, parse transcript response |
| `claude.rs` | Add `--attachment <path>` flag support in cold-start path; extend stream-json input for image paths |
| `agent.rs` | Pass image attachment path from trigger metadata to Claude client; cleanup temp files post-response |
| `nv-core/config.rs` | No changes needed — `ELEVENLABS_API_KEY` already available in Secrets |

## Risks
| Risk | Mitigation |
|------|-----------|
| Telegram photo file size limit (20MB download via Bot API) | Largest PhotoSize is typically 1-3MB. Log warning if download fails. |
| Claude CLI `--attachment` flag availability | Verify at startup or first photo. Fall back to describing the photo if flag unavailable. |
| ElevenLabs STT API latency | Send typing indicator while transcribing. Text reply path unaffected. |
| ElevenLabs STT quota/cost | Audio files are rarer than voice notes. Log usage for monitoring. |
| Temp file leak on crash | Use `/tmp` (OS cleans on reboot). Name prefix `nv-photo-` for manual cleanup. |
| Large audio files (podcasts) | Telegram limits file downloads to 20MB via Bot API. Long audio transcription may timeout — add 30s timeout. |
