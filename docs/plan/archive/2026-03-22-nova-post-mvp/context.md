# Context: Nova Post-MVP Plan

## Context Tag

system

## Current State (v1 MVP — COMPLETE)

**Delivered:** 10 original specs + 2 bonus specs (bootstrap-soul, message-store). 13.7K LOC Rust,
307 tests, running as systemd service. Bootstrap completed. All planning artifacts locked.

**Runtime:** Active on Telegram (@Nova_acosta_bot), Jira connected (leonardoacosta.atlassian.net),
Nexus configured (2 agents, currently disconnected), SQLite message store with analytics,
proactive digest with notification gating.

**Beyond Roadmap (bonus):** Claude CLI OAuth, sandbox isolation, filesystem tools, Discord/Teams
relay bots, Markdown-to-HTML, thinking ticker, edit-or-fallback delivery, 2G OOM fix.

## Codebase Structure

```
nv/ (13.7K LOC Rust + 191 LOC Python relays)
├── crates/
│   ├── nv-core/      (613 LOC)  — Types, config, Channel trait
│   ├── nv-daemon/    (12.6K LOC) — Agent loop, all integrations
│   └── nv-cli/       (504 LOC)  — CLI: status, ask, config, digest, stats
├── config/           — system-prompt, soul, identity, user, bootstrap, env
├── deploy/           — nv.service, install.sh
├── relays/           — Discord bot (Python), Teams webhook (Python)
├── proto/            — nexus.proto (gRPC)
└── openspec/         — 9 archived, 5 open specs
```

## Open Work Inventory

### Tier 1: Deferred Tasks from Completed MVP Specs (15 tasks)

**Jira (12 tasks):** Retry wrapper, callback handlers (edit/cancel/expiry), HTTP mock tests,
integration tests, manual e2e gate.

**Telegram (2 tasks):** Integration test + manual e2e gate.

**Nexus (1 task):** Wire error callbacks in Telegram handler.

### Tier 2: Written Specs — Not Yet Applied (2 specs)

| Spec | Tasks | Effort | Ready? |
|------|-------|--------|--------|
| `add-interaction-diary` | 9 | S | Yes |
| `add-voice-reply` | 14 | M | Yes (needs ffmpeg + ElevenLabs key) |

### Tier 3: Planned P1 Specs — Not Yet Written (4 specs)

| Spec | Purpose | Interim |
|------|---------|---------|
| `discord-channel` | Native WebSocket + REST adapter | Relay bot exists |
| `teams-channel` | MS Graph API + OAuth2 | Webhook relay exists |
| `imessage-channel` | BlueBubbles API or Mac relay | None |
| `jira-webhooks` | Inbound bidirectional sync | One-way (NV→Jira) works |

### Tier 4: Architectural Improvements (not spec'd)

- `/apply` language agnosticity (extract T3-specific config to reference file)
- CLI subprocess latency (~8-14s per turn, architectural limitation)
- Persistent Claude session (vs cold-start per turn)
- Embedding-based memory search (currently grep-only)

## What Nova Has Learned (from Bootstrap + Memory)

- Leo runs 14+ projects (oo, tc, tl, mv, ss, co, cl, cw, la, hl, if, dc, cx, nv)
- P0-P1 notifications only, suppress routine noise
- Morning digests 1-2x daily
- Wants Nova proactively researching, not just reporting status
- One upset client = immediate escalation
- Commands on request, not by default
- Terse responses, lead with the answer

## Discovery Metadata

- **Project**: nv (Nova)
- **Path**: /home/nyaptor/nv
- **Timestamp**: 2026-03-22T13:15:00-05:00
- **Quick mode**: false
- **Detected mode**: system
- **MVP status**: Complete (10 original + 2 bonus specs, 307 tests)
- **Runtime status**: Active (uptime 1h+, Telegram connected)
