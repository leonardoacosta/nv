# Proposal: voice-to-text-stt

## Change ID
`voice-to-text-stt`

## Summary

Add Deepgram nova-2 as the STT provider for Telegram voice messages. Currently voice notes
are transcribed via ElevenLabs (`transcribe_audio_elevenlabs` in `speech_to_text.rs`). This
spec adds a parallel `transcribe_audio_deepgram()` function and switches the voice-note path
to use it, while the audio-file path (MP3/WAV sent via the audio player) continues using
ElevenLabs. Large files (>20 MB) are rejected before download.

## Context

- `crates/nv-daemon/src/speech_to_text.rs` — ElevenLabs STT client; extend with Deepgram function
- `crates/nv-daemon/src/channels/telegram/mod.rs` — `transcribe_voice_message()` uses
  `transcribe_audio_elevenlabs`; switch it to Deepgram
- `crates/nv-daemon/src/channels/telegram/types.rs` — `Voice` struct already has `file_size: Option<i64>`; used for the >20 MB gate
- `crates/nv-core/src/config.rs` — `AgentConfig`; add optional `deepgram_model` (default: `"nova-2"`)
- Wave: Phase 2 Voice Channels, Wave 5

## Motivation

ElevenLabs STT uses a multipart upload which adds latency and per-character billing. Deepgram
nova-2 accepts a raw audio body POST, returns a transcript in ~1–2 s for short clips, and
is purpose-built for real-time transcription. Voice notes (OGG) and audio files (MP3/WAV)
have different characteristics; keeping them on separate providers lets each use the best fit.

## Requirements

### Req-1: DeepgramClient — `transcribe_audio_deepgram()`

Add a new public async function to `crates/nv-daemon/src/speech_to_text.rs`:

```rust
pub async fn transcribe_audio_deepgram(
    audio_bytes: Vec<u8>,
    mime_type: &str,
    api_key: &str,
    model: &str,      // e.g. "nova-2"
) -> Result<String>
```

HTTP contract:

```
POST https://api.deepgram.com/v1/listen?model={model}&smart_format=true
Authorization: Token {api_key}
Content-Type: {mime_type}
Body: <raw audio bytes>
```

Response parsing — extract `results.channels[0].alternatives[0].transcript`:

```json
{
  "results": {
    "channels": [{
      "alternatives": [{ "transcript": "..." }]
    }]
  }
}
```

Error cases (all return `Err(anyhow!(...))` so the caller can send the user-facing reply):

| Condition | Error message |
|-----------|--------------|
| HTTP non-2xx | `"Deepgram STT returned {status}: {body}"` |
| JSON parse failure | `"failed to parse Deepgram STT response: {e}"` |
| Missing transcript path | `"Deepgram STT response missing transcript field: {json}"` |
| Empty transcript | `"Deepgram STT returned empty transcript"` |

Request timeout: 30 seconds (matches existing ElevenLabs constant).

### Req-2: Large-file gate (>20 MB reject)

In `transcribe_voice_message()` in `mod.rs`, before calling `get_file()`:

1. Read `file_size` from `msg.metadata["file_size"]` (populated by `to_inbound_message()` from
   `Voice.file_size`).
2. If `file_size > 20 * 1024 * 1024` (20 971 520 bytes), send a Telegram reply:
   `"Voice message too large to transcribe (max 20 MB)."` and return `Err`.

Note: `Voice.file_size` is `Option<i64>` — if absent, skip the gate and proceed (Telegram
only omits `file_size` for very small files).

### Req-3: Switch `transcribe_voice_message()` to Deepgram

In `crates/nv-daemon/src/channels/telegram/mod.rs`, `transcribe_voice_message()`:

- Replace the `ELEVENLABS_API_KEY` env-var check with `DEEPGRAM_API_KEY`.
- Replace the `transcribe_audio_elevenlabs(...)` call with `transcribe_audio_deepgram(...)`.
- Pass `model` from `AgentConfig.deepgram_model` (default `"nova-2"`). The channel does not
  currently hold a config reference; pass `model` as a `&str` parameter to
  `transcribe_voice_message(channel, msg, model)` and thread it through from `run_poll_loop`
  which already receives a `voice_enabled: Arc<AtomicBool>`. Add `model: String` to the
  signature (cloned from config at startup).
- User-facing error messages (unchanged from existing):
  - Missing key: `"Voice transcription not configured (DEEPGRAM_API_KEY missing)."`
  - Empty transcript: `"No speech detected in voice message."`
  - Other errors: `"Could not transcribe voice message. Please try again or type your message."`

The `handle_audio_message()` path (MP3/WAV audio files) remains on ElevenLabs — no change.

### Req-4: Config — `deepgram_model`

In `crates/nv-core/src/config.rs`, add to `AgentConfig`:

```rust
/// Deepgram model for voice message transcription. Defaults to "nova-2".
#[serde(default = "default_deepgram_model")]
pub deepgram_model: String,
```

```rust
fn default_deepgram_model() -> String {
    "nova-2".to_string()
}
```

No `config.toml` changes required — the default covers the common case.

### Req-5: Doppler — `DEEPGRAM_API_KEY`

Add `DEEPGRAM_API_KEY` to the `nv-daemon` Doppler config (dev + prod environments). The key
is read at runtime via `std::env::var("DEEPGRAM_API_KEY")`. The engineer implementing this
spec does NOT add the key; it is a [user] task.

## Scope

- **IN**: `transcribe_audio_deepgram()`, large-file gate, switch voice-note path to Deepgram, `deepgram_model` config field
- **OUT**: Switching audio-file path (stays ElevenLabs), Whisper fallback, streaming transcription, multi-language detection, audio storage

## Impact

| File | Change |
|------|--------|
| `crates/nv-daemon/src/speech_to_text.rs` | Add `transcribe_audio_deepgram()` |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Add file-size gate, switch `transcribe_voice_message()` to Deepgram, add `model` param to `run_poll_loop` |
| `crates/nv-core/src/config.rs` | Add `deepgram_model` field + default fn |

## Risks

| Risk | Mitigation |
|------|-----------|
| Deepgram API latency | ~1–2 s for short clips; typing indicator already sent before the call |
| `DEEPGRAM_API_KEY` not set | Graceful degradation — reply sent, error logged, message not dispatched |
| OGG format rejected by Deepgram | nova-2 natively supports OGG Opus (Telegram's format); no conversion needed |
| File size unavailable | Gate is skipped when `file_size` is absent — no false rejections |
| Cost | ~$0.0043/min; at 10 msgs/day × 30 s avg ≈ $0.04/month — negligible |
