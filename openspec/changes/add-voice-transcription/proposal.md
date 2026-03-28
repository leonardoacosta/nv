# Proposal: Add Voice Transcription

## Change ID
`add-voice-transcription`

## Summary

Add ElevenLabs STT transcription to the daemon's voice message path so that inbound Telegram voice
messages produce populated `text`/`content` fields before entering the routing cascade. Voice
messages are currently normalized with empty strings — keyword matching, embedding routing, and the
Agent SDK all receive empty prompts, making voice a silently broken channel.

## Context
- Breaks at: `packages/daemon/src/channels/telegram.ts` — `normalizeVoiceMessage()` (line 139)
  sets `text: ""` and `content: ""` after resolving `fileUrl`; no STT call is made
- Downstream: `packages/daemon/src/brain/router.ts` — `MessageRouter.route(text)` receives `""`
  and falls through all tiers with zero signal
- Convention: ElevenLabs is the project-mandated audio vendor for both TTS and STT
  (`feedback_elevenlabs_only.md`); `ELEVENLABS_API_KEY` is in Doppler
- Related: archived `add-voice-to-text` proposal (used Deepgram — superseded by this change)
- Depends on: none

## Motivation

Voice messages are a first-class Telegram interaction: a single tap-and-hold is faster than typing
for short commands. The infrastructure to receive them already exists — `normalizeVoiceMessage()`
resolves a `fileUrl` from the Bot API and sets `type: "voice"` — but then discards the audio with
empty text. The result is a feature that appears to work (no error is raised) but silently delivers
nothing to the agent.

Fixing this requires one targeted addition: an STT step inserted between URL resolution and message
emission that downloads the voice file and calls ElevenLabs `/speech-to-text/convert`, then writes
the transcript into `text` and `content`. The routing cascade, Agent SDK, and all downstream
consumers require no changes — they already handle non-empty text correctly.

## Requirements

### Req-1: ElevenLabs STT Client

Create `packages/daemon/src/features/stt/client.ts` exposing a `transcribe(fileUrl: string):
Promise<string>` function. The function MUST:
- Download the audio bytes from the provided Telegram CDN URL via HTTP GET
- POST the raw bytes to `https://api.elevenlabs.io/v1/speech-to-text/convert` with
  `Content-Type: audio/ogg` and `xi-api-key: <ELEVENLABS_API_KEY>` header
- Parse the `text` field from the JSON response
- Throw a typed `SttError` on network failure, non-2xx response, or empty transcript

### Req-2: Environment Variable

`ELEVENLABS_API_KEY` is already provisioned in Doppler for the existing TTS path. The STT client
MUST read it from `process.env["ELEVENLABS_API_KEY"]` at call time (not at module load) and throw
`SttError` with message `"ELEVENLABS_API_KEY not set"` if absent.

### Req-3: Wire STT into `normalizeVoiceMessage`

After resolving `fileUrl`, `normalizeVoiceMessage()` MUST call `transcribe(fileUrl)` and write the
result into the returned `Message` as both `text` and `content`. The function signature and return
type remain unchanged (`Promise<Message>`). The `fileUrl` MUST still be preserved in `metadata`
for audit purposes.

### Req-4: Graceful Degradation on Failure

When transcription fails (missing key, download error, ElevenLabs API error, empty transcript),
`normalizeVoiceMessage()` MUST NOT throw. It MUST log the error at `warn` level and return the
message with `text` and `content` set to `"[Voice message — transcription unavailable]"` so the
routing cascade receives a non-empty, human-readable fallback rather than silently empty text.

### Req-5: No fileUrl — Skip and Fallback

If `fileUrl` could not be resolved (the `bot.getFileLink()` call in the existing catch block
failed), `normalizeVoiceMessage()` MUST skip the STT call entirely and set `text`/`content` to
`"[Voice message — could not retrieve audio file]"` at `warn` log level.

## Scope
- **IN**: `packages/daemon/src/features/stt/client.ts` (new), `normalizeVoiceMessage()` wiring,
  `SttError` class, env var read pattern, graceful degradation, unit tests
- **OUT**: STT for photo captions, config flag to disable STT, custom ElevenLabs model selection,
  audio caching, streaming transcription, language detection, transcription stored to DB

## Impact
| Area | Change |
|------|--------|
| `packages/daemon/src/features/stt/client.ts` (new) | ElevenLabs STT HTTP client, `SttError`, `transcribe()` |
| `packages/daemon/src/channels/telegram.ts` | Wire `transcribe()` in `normalizeVoiceMessage()`, handle errors |

## Risks
| Risk | Mitigation |
|------|-----------|
| ElevenLabs STT API latency (~1-3s) | Voice messages already have an async normalization step; Telegram bot users expect slight delay for voice. Send typing indicator if latency becomes noticeable. |
| ElevenLabs STT API unavailable | Req-4 graceful degradation: fallback text instead of crash. Downstream agent receives non-empty prompt. |
| `ELEVENLABS_API_KEY` not in Doppler dev config | Req-2 throws `SttError` caught by Req-4; logs `warn`; user receives fallback reply. No silent failure. |
| OGG/Opus MIME type mismatch | ElevenLabs STT accepts `audio/ogg` for Telegram voice. If rejection observed, re-test with `audio/oga`; update `Content-Type` constant. |
| Very long voice messages (>60s) cause timeout | Set HTTP timeout of 30s on both download and STT POST. Long messages time out and hit Req-4 fallback. |
