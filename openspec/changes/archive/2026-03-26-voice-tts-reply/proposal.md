# Proposal: Voice TTS Reply

## Change ID
`voice-tts-reply`

## Summary
Refine TTS voice delivery so Nova only synthesizes a voice reply when the original inbound message
was itself a voice note. Currently, `voice_enabled` fires TTS on every response below the
character threshold regardless of how the user sent their message. This spec makes TTS
context-aware: voice in → voice out.

## Context
- Phase 2 — Voice Channels | Wave 6
- Depends on: `voice-to-text-stt` (Wave 5) — ElevenLabs STT and `speech_to_text.rs` already
  implemented; Telegram poll loop already detects `metadata["voice"] == true` for inbound voice
  notes and transcribes them before dispatch
- Prior spec `add-voice-reply` (archived 2026-03-22) delivered: `tts.rs` (ElevenLabs + ffmpeg),
  `send_voice` on `TelegramClient`, `voice_enabled` AtomicBool toggle, character threshold gate
- Gap: the current TTS delivery block in `worker.rs` (lines 1556–1590) triggers on
  `voice_enabled && len <= voice_max_chars` only — it has no awareness of whether the originating
  trigger was a voice message

## Motivation
Unconditional TTS is jarring. A user typing a quick text question does not expect an audio reply
back. Voice-in → voice-out is the natural contract: the user chose to speak, so Nova responds in
kind. Text-in → text-out remains unchanged. This also reduces unnecessary ElevenLabs API calls and
ffmpeg load.

## Requirements

### Req-1: Voice-Trigger Propagation
The `WorkerTask` struct gains a new boolean field `is_voice_trigger`. The orchestrator sets this
field to `true` when any trigger in the batch carries `metadata["voice"] == true` (the flag
already set by the Telegram poll loop on inbound voice notes). Field defaults to `false` for all
other trigger sources (text, CLI, timers, watchers).

### Req-2: Conditional TTS Gate
The voice delivery block in `worker.rs` adds `task.is_voice_trigger` as a precondition alongside
the existing `voice_enabled` and character-threshold checks. The full gate becomes:

```
voice_enabled && is_voice_trigger && response_text.len() <= voice_max_chars && !response_text.is_empty()
```

### Req-3: Tool-Call Suppression
When the response involved one or more tool calls (`tool_names` is non-empty), TTS is skipped
even if the trigger was a voice note. Tool-heavy responses (search results, task lists, code
output) are not suitable for voice synthesis. The gate becomes:

```
voice_enabled && is_voice_trigger && tool_names.is_empty() && response_text.len() <= voice_max_chars && !response_text.is_empty()
```

### Req-4: Dual Delivery Preserved
When TTS fires, the text reply is still sent first (existing behavior). The voice note follows
asynchronously. If synthesis or upload fails, it is logged at WARN level and does not affect
the already-delivered text reply.

### Req-5: Caption on Voice Message
When `send_voice` is called, pass the response text as a `caption` field (Telegram supports up
to 1024 chars as caption on voice messages). This gives users the text inline with the audio
bubble — no separate follow-up message needed. If the text exceeds 1024 chars (unlikely given
the 500-char threshold), truncate the caption to 1024 chars with an ellipsis.

## Design

### WorkerTask field addition

```rust
pub struct WorkerTask {
    // ... existing fields ...
    /// True when the originating trigger was a Telegram voice note.
    /// Controls whether Nova responds with a synthesized voice message.
    pub is_voice_trigger: bool,
}
```

### Orchestrator: detect voice origin

```rust
// In process_trigger_batch, before constructing WorkerTask:
let is_voice_trigger = triggers.iter().any(|t| {
    if let Trigger::Message(msg) = t {
        msg.metadata.get("voice").and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        false
    }
});

let task = WorkerTask {
    // ...
    is_voice_trigger,
};
```

### Worker: updated TTS gate

```rust
if deps.voice_enabled.load(Ordering::Relaxed)
    && task_is_voice_trigger          // captured before run_worker executes
    && tool_names.is_empty()
    && (response_text.len() as u32) <= deps.voice_max_chars
    && !response_text.is_empty()
{
    // synthesize + send_voice (unchanged async spawn)
}
```

### send_voice caption

The `TelegramClient::send_voice` signature gains an optional `caption: Option<&str>` parameter.
The multipart form appends `caption` and `parse_mode` fields when `Some`. The caption is passed
from the worker as `Some(&response_text)` (Telegram truncation handled client-side).

## Scope
- **IN**: `WorkerTask.is_voice_trigger` field, orchestrator voice-origin detection, TTS gate
  refinement (add `is_voice_trigger` + `tool_names.is_empty()`), `send_voice` caption parameter
- **OUT**: Voice replies for proactive/digest messages, TTS for non-Telegram channels, voice
  toggle per-chat (still global `AtomicBool`), streaming TTS, voice for tool-heavy responses

## File Impact
| File | Change |
|------|--------|
| `crates/nv-daemon/src/worker.rs` | Add `is_voice_trigger` to `WorkerTask`; update TTS gate to require it + `tool_names.is_empty()` |
| `crates/nv-daemon/src/orchestrator.rs` | Detect voice origin from trigger metadata; set `is_voice_trigger` in `WorkerTask` construction |
| `crates/nv-daemon/src/channels/telegram/client.rs` | Add `caption: Option<&str>` to `send_voice`; append to multipart form |

## Risks
| Risk | Mitigation |
|------|-----------|
| Trigger batch mixes voice + text messages | `any()` over triggers is correct — if any trigger is a voice note, reply voice. Batch-mixing is rare (Telegram sends one message per update) |
| Caption truncation surprises user | 500-char TTS threshold is well below the 1024-char Telegram caption limit; truncation is a safety net, not normal path |
| Downstream callers of `send_voice` break on signature change | One internal call site in `worker.rs` — update it in the same task |
