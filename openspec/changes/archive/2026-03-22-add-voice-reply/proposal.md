# Proposal: Add Voice Reply

## Change ID
`add-voice-reply`

## Summary
Enable Nova to respond with Telegram voice messages (OGG/Opus waveform bubbles) alongside text replies, with a configurable toggle and character-length threshold.

## Context
- Extends: `crates/nv-daemon/src/telegram/client.rs` (add `send_voice`), `crates/nv-daemon/src/agent.rs` (voice routing after text), `crates/nv-core/src/config.rs` (voice settings)
- Related: `tts_url` config field exists in `DaemonConfig` but is unused; ElevenLabs API key available in environment

## Motivation
Nova currently replies only in text. Voice replies make interactions more natural — especially for short status updates, confirmations, and digest summaries. The user's homelab TTS service (claude-notify) already uses ElevenLabs for local audio playback; this extends the same capability to Telegram.

## Requirements

### Req-1: TTS Synthesis
The daemon calls the ElevenLabs TTS API directly to synthesize response text into audio bytes. The API key is read from the `ELEVENLABS_API_KEY` environment variable. The voice ID and model are configurable via `nv.toml`. Audio is returned as MP3 from ElevenLabs and transcoded to OGG/Opus via `ffmpeg` before upload.

### Req-2: Telegram Voice Upload
A new `TelegramClient::send_voice(chat_id, ogg_bytes, reply_to)` method uploads OGG/Opus audio to Telegram's `sendVoice` endpoint using multipart/form-data. The voice message appears as an inline waveform bubble in the chat.

### Req-3: Voice Toggle
Voice replies are controlled by two layers:
1. **Config default**: `[daemon] voice_enabled = true/false` in `nv.toml` (default: `false`)
2. **Runtime override**: A `/voice` Telegram command toggles an `AtomicBool` that overrides the config value for the current daemon session. State resets on restart.

### Req-4: Character Threshold
Responses exceeding `voice_max_chars` (default: 500, configurable in `nv.toml`) skip voice synthesis. Only the text reply is sent. This prevents unwieldy long voice messages.

### Req-5: Dual Delivery
When voice is enabled and the response is within the character threshold, Nova sends the text reply first (existing behavior), then follows with a voice message. If TTS or voice upload fails, the text reply has already been delivered — the failure is logged but does not affect the user.

## Scope
- **IN**: ElevenLabs TTS client, OGG/Opus transcoding via ffmpeg, `sendVoice` Telegram method, config/runtime toggle, character threshold, dual text+voice delivery
- **OUT**: Voice-to-text (inbound voice recognition), `sendAudio` (music player format), custom TTS service abstraction (ElevenLabs only for now), voice for digest/proactive messages (future), TUI toggle (future — daemon-only for now)

## Impact
| Area | Change |
|------|--------|
| nv-core/config.rs | Add `voice_enabled`, `voice_max_chars`, `elevenlabs_voice_id`, `elevenlabs_model` to DaemonConfig |
| nv-core/types.rs | No change — OutboundMessage stays text-only; voice is a post-send enhancement |
| nv-daemon/tts.rs | New module: ElevenLabs HTTP client + ffmpeg transcoding |
| nv-daemon/telegram/client.rs | Add `send_voice` method (multipart upload) |
| nv-daemon/agent.rs | After text reply, conditionally synthesize + send voice |
| nv-daemon/main.rs | Wire voice toggle AtomicBool, handle `/voice` command |

## Risks
| Risk | Mitigation |
|------|-----------|
| ElevenLabs API latency adds delay after text reply | Voice sent asynchronously — text already delivered, voice follows when ready |
| ElevenLabs rate limits or quota exhaustion | Log error, degrade gracefully to text-only |
| ffmpeg not installed on deployment target | Check at startup, disable voice with warning if missing |
| OGG/Opus encoding failures | Fallback: skip voice for that response, log error |
| Large text → large audio → slow upload | Character threshold (500 chars default) prevents this |
