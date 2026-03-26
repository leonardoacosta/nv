# Scope Lock -- Nova v8

## Vision

Ship all 17 backlog ideas + deferred streaming delivery in a single mega-session, completing Nova's
transition from reactive CLI wrapper to proactive multi-channel AI assistant with voice, contacts,
and autonomous behavior.

## Target Users

Leo (sole operator). Nova serves Leo across Telegram (primary), Discord, Teams, and the dashboard.

## Domain

Nova's daemon (nv-daemon), core library (nv-core), CLI (nv-cli), and dashboard (apps/dashboard/).
No external services beyond existing Tailscale homelab.

## v8 Must-Do

All 18 features organized into 5 waves:

### Wave 1 -- Telegram UX (6 specs)
Priority: Where Leo interacts with Nova most. Streaming delivery is the #1 deferred item from v7.

1. **streaming-response-delivery** -- "..." placeholder + progressive Telegram edits every 1.5s.
   Completes deferred Req-2 from response-latency-optimization. (nv-vvq carry-forward)
2. **callback-handler-completion** -- Complete Telegram inline keyboard callbacks: edit, cancel,
   expiry, snooze actions on obligation/reminder buttons. (nv-l2e)
3. **telegram-streaming-styled-buttons** -- Bot API 9.3+ features: sendMessageEffect, styled text,
   inline query improvements. (nv-32o)
4. **telegram-reminder-ux** -- Mark done, remind later, backlog from reminder notifications.
   Inline keyboard integration with obligation store. (nv-b9t)
5. **telegram-bot-presence** -- Investigate and implement bot presence/typing indicators if
   supported by Bot API. (nv-mkp)
6. **error-recovery-ux** -- Better error messages when Claude fails mid-conversation. Structured
   error types, retry suggestions, graceful degradation. (nv-dkv)

### Wave 2 -- Voice Channels (2 specs)
Priority: Hands-free Nova. API keys already in Doppler.

7. **voice-to-text-stt** -- Telegram voice message transcription via Deepgram API. Forward
   transcribed text through normal message pipeline. (nv-dnq)
8. **voice-tts-reply** -- ElevenLabs TTS for Telegram voice replies. Nova responds with OGG/Opus
   voice notes when user sends voice. (nv-4an)

### Wave 3 -- Data & Integrations (4 specs)
Priority: Nova knows more context about who Leo talks to and what tools are available.

9. **contact-profiles-system** -- Full system: SQLite contacts table, sender FK migration across
   all channels, contact/*.md profiles, relationship types (work/personal-client/contributor/
   social), dashboard contacts page. (nv-0bxt)
10. **ms-graph-cli-tools** -- Rust CLI + daemon tools for Outlook (read email/calendar), Teams
    (enhanced), ADO (pipelines/builds). Dual auth: device-code for CLI, client-credentials for
    daemon. ADO spec already designed. (nv-rctm)
11. **cross-channel-routing** -- send_to_channel and list_channels tools. Nova can route messages
    between Telegram/Discord/Teams programmatically. (nv-2e6)
12. **tool-result-caching** -- Cache tool results within a session to avoid redundant API calls.
    TTL-based with invalidation on write operations. (nv-4pq)

### Wave 4 -- Autonomy (3 specs)
Priority: Nova does things without being asked.

13. **proactive-followups** -- Nova follows up on stale obligations and commitments. Watcher-based
    with configurable thresholds. (nv-7xc)
14. **proactive-obligation-research** -- Nova researches obligations autonomously with full read
    access to relevant tools. Background research before Leo asks. (nv-lvm)
15. **self-improvement-research** -- Nova analyzes its own performance, identifies patterns in
    failures, and suggests improvements. Meta-cognition capability. (nv-ad8)

### Wave 5 -- Polish (3 specs)
Priority: Remaining quality-of-life improvements.

16. **agent-persona-switching** -- Per-channel persona profiles. Nova adapts tone and behavior
    based on channel context (work Teams vs personal Telegram). (nv-n2l)
17. **interaction-diary** -- Zero-cost daily interaction log at ~/.nv/diary/YYYY-MM-DD.md. Every
    trigger produces a human-readable diary entry. (nv-7n4)
18. **persistent-subprocess-fix** -- Complete deferred Req-3 from response-latency-optimization:
    diagnose and fix CC stream-json subprocess hang, enable persistent sessions. (carry-forward)

## v8 Won't-Do

- Dashboard authentication (Tailscale isolation sufficient)
- Dashboard redesign (v7 rebuild is fresh)
- New channel integrations beyond MS Graph (no Slack, no email)
- Multi-user support (single-operator architecture)
- Public-facing APIs or deployments
- Mobile app (Telegram IS the mobile interface)

## Business Model

Personal tool. No monetization. Homelab deployment via Docker + Tailscale.

## Scale Target

1 user (Leo), ~50-100 messages/day across channels, ~5 active CC sessions concurrent.

## Hard Constraints

- Tailscale-only network access (no public exposure)
- Doppler for all secrets (Deepgram, ElevenLabs, MS Graph keys already provisioned)
- SQLite for all local storage (no external databases)
- Rust for daemon, TypeScript for dashboard (no new languages)
- Single mega-session delivery model

## Timeline

No external deadline. Self-paced, single session.

## Assumptions Corrected

- v7 assumed dashboard auth was needed before use -> Leo says skip entirely, Tailscale is sufficient
- v7 deferred streaming as complex -> v8 treats it as Wave 1 priority since Telegram UX is primary
- Contact system initially scoped as "explore" -> Leo wants full implementation (schema + migration + UI)
- Voice channels assumed account setup needed -> keys already in Doppler, ready to implement
