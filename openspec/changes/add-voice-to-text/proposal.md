# Proposal: Add Voice-to-Text

## Change ID
`add-voice-to-text`

## Summary

Receive inbound Telegram voice messages, transcribe them via the Deepgram API, and inject the
transcribed text as a regular `Trigger::Message`. Completes bidirectional voice: Nova already
speaks (TTS via ElevenLabs), now it listens.

## Context
- Extends: `crates/nv-daemon/src/telegram/mod.rs` (voice message detection in poll_messages), `crates/nv-daemon/src/telegram/client.rs` (getFile download, Deepgram HTTP call), `crates/nv-daemon/src/telegram/types.rs` (voice message fields)
- Related: Existing TTS outbound in `tts.rs` (ElevenLabs), `InboundMessage` type, voice config in `AgentConfig`
- Depends on: none

## Motivation

Leo frequently wants to send quick instructions while away from a keyboard. Telegram natively
supports voice messages — a single tap-and-talk interaction. Transcribing these to text lets
Nova process them identically to typed messages, with zero changes to the agent loop, tools, or
worker logic.

## Requirements

### Req-1: Voice Message Detection

Detect voice messages in the Telegram update stream. Telegram sends voice messages as `Update`
objects with a `voice` field containing:

```json
{
    "voice": {
        "file_id": "...",
        "file_unique_id": "...",
        "duration": 5,
        "mime_type": "audio/ogg",
        "file_size": 12345
    }
}
```

Update the `Update` and `to_inbound_message()` parsing to detect voice messages and flag them
for transcription before trigger dispatch.

### Req-2: Voice File Download

Download the voice file from Telegram using the Bot API:

1. Call `getFile(file_id)` → returns `File { file_path: "voice/file_123.oga" }`
2. Download from `https://api.telegram.org/file/bot{token}/{file_path}`
3. Store in memory as `Vec<u8>` — no disk write needed

Add `get_file(file_id)` and `download_file(file_path)` methods to `TelegramClient`.

### Req-3: Deepgram Transcription

POST the audio bytes to Deepgram's speech-to-text API:

```
POST https://api.deepgram.com/v1/listen?model=nova-2&smart_format=true
Authorization: Token {DEEPGRAM_API_KEY}
Content-Type: audio/ogg
Body: <raw audio bytes>
```

Parse the response JSON:
```json
{
    "results": {
        "channels": [{
            "alternatives": [{
                "transcript": "Check the status of tribal cities"
            }]
        }]
    }
}
```

Extract the transcript string. If empty or API error, send error to Telegram:
"Could not transcribe voice message. Please try again or type your message."

### Req-4: Inject as Trigger::Message

Create an `InboundMessage` with:
- `content`: the transcribed text
- `channel`: "telegram"
- `sender`: original voice message sender
- `metadata`: include `"voice": true` and `"duration_secs": N` for context

Push into the trigger channel as `Trigger::Message` — the agent loop processes it identically
to a typed message. Claude receives the text with no indication it was voice (unless it checks
metadata).

### Req-5: Configuration

Add `DEEPGRAM_API_KEY` to the environment. No config file change needed — read directly from
env var. If the key is not set, voice messages get a reply: "Voice transcription not configured."

Optional config in `[agent]`:
```toml
deepgram_model = "nova-2"  # default: nova-2
```

## Scope
- **IN**: Voice message detection, Telegram file download, Deepgram transcription, text injection as Trigger::Message, DEEPGRAM_API_KEY config
- **OUT**: Multi-language detection, voice message forwarding, audio file storage, Whisper fallback, voice-to-voice (skip text), streaming transcription

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/telegram/types.rs` | Add Voice struct, voice field to Update, update to_inbound_message() |
| `crates/nv-daemon/src/telegram/client.rs` | Add get_file(), download_file(), transcribe_voice() methods |
| `crates/nv-daemon/src/telegram/mod.rs` | Detect voice messages in poll_messages, transcribe before dispatch |
| `crates/nv-core/src/config.rs` | Add optional deepgram_model to AgentConfig |

## Risks
| Risk | Mitigation |
|------|-----------|
| Deepgram API latency | ~1-2s for short messages. Acceptable — send typing indicator while transcribing. |
| Deepgram API cost | ~$0.0043/min. At ~10 voice msgs/day, 30s avg = ~$0.04/month. Negligible. |
| Poor transcription accuracy | Deepgram Nova-2 is state-of-the-art. For ambiguous transcriptions, Claude can ask for clarification. |
| Voice messages with no speech | Deepgram returns empty transcript. Handle gracefully: "No speech detected in voice message." |
| DEEPGRAM_API_KEY not set | Graceful degradation — reply "Voice transcription not configured" |
