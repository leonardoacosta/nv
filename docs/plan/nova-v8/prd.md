# PRD -- Nova v8

> Lean PRD for engineering-focused phase. Derived directly from scope-lock.md.
> No user stories, financials, or brand artifacts needed.

## Summary

Complete Nova's evolution from reactive CLI wrapper to proactive multi-channel AI assistant.
Ship 18 features: streaming Telegram responses, voice channels (STT/TTS), contact profiles system,
MS Graph integrations, autonomous follow-ups, and persona switching. Single mega-session delivery.

## Architecture

```
Telegram (primary) ──┐
Discord ─────────────┤
Teams ───────────────┤──> nv-daemon (Axum + SQLite)
Voice (Deepgram/11L) ┤    ├── Worker pool (cold-start / persistent / HTTP API)
CLI ─────────────────┘    ├── CcSessionManager (team agents)
                          ├── ContactStore (new v8)
                          ├── ToolResultCache (new v8)
                          └── ProactiveWatcher (new v8)

apps/dashboard/ (Next.js 15) ──> /api/* proxy to daemon
```

## Feature Areas

| Wave | Area | Specs | Files |
|------|------|-------|-------|
| 1 | Telegram UX | 6 | worker.rs, telegram/client.rs, claude.rs |
| 2 | Voice | 2 | telegram/client.rs, new voice modules |
| 3 | Data/Integrations | 4 | new stores, tools/*, config |
| 4 | Autonomy | 3 | orchestrator.rs, watchers, new modules |
| 5 | Polish | 3 | worker.rs, diary.rs, claude.rs |

## Dependencies

- Wave 1 streaming-response-delivery must complete before voice-tts-reply (Wave 2)
- contact-profiles-system (Wave 3) is independent of all other specs
- proactive-followups (Wave 4) depends on obligation_store improvements from Wave 1 callback work
- persistent-subprocess-fix (Wave 5) depends on streaming-response-delivery (Wave 1)

## External Services

| Service | API | Auth | Status |
|---------|-----|------|--------|
| Deepgram | STT transcription | API key in Doppler | Ready |
| ElevenLabs | TTS synthesis | API key in Doppler | Ready |
| MS Graph | Outlook/Teams/ADO | Client credentials + device code | Partial (Teams exists) |
