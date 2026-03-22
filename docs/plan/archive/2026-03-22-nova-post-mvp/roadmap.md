# Roadmap — Nova Post-MVP

> Generated from post-MVP PRD. 10 specs across 3 phases, 5 waves.
> Execution order: Architecture → Channels → Hardening

---

## Wave 1: Architecture — Persistent Session (Mon)

### Spec 1: `persistent-claude-session`

**Type:** feature | **Effort:** L | **Deps:** none

Replace cold-start `claude -p` per turn with a long-lived CLI subprocess using
`--input-format stream-json` + `--output-format stream-json`. Keep the subprocess
alive between turns. Pipe messages via stdin, parse events from stdout.

- Refactor `ClaudeClient` to manage a persistent `tokio::process::Child`
- Stdin writer for sending messages (JSON stream format)
- Stdout reader parsing stream-json events (text, tool_use, result, done)
- Auto-restart with backoff on subprocess death
- Fallback to cold-start mode if persistent session fails
- Update agent loop to use persistent client
- Measure: response time before/after

**Gate:** Response time consistently under 3s for simple queries.

---

## Wave 2: Architecture — Features (Tue)

### Spec 2: `add-interaction-diary` (already written)

**Type:** feature | **Effort:** S | **Deps:** none | **Status:** Spec exists, apply only

9 tasks. Rust-written daily log at `~/.nv/diary/YYYY-MM-DD.md`.

### Spec 3: `add-voice-reply` (already written)

**Type:** feature | **Effort:** M | **Deps:** none | **Status:** Spec exists, apply only

14 tasks. ElevenLabs TTS → Telegram voice messages. Requires ffmpeg + API key.

---

## Wave 3: Channels — Independent (Wed-Thu)

All channel specs are independent (no file conflicts) — can execute in parallel.

### Spec 4: `discord-channel`

**Type:** feature | **Effort:** M | **Deps:** persistent-claude-session (benefits from fast response)

Native Discord gateway adapter.
- Discord gateway WebSocket (tokio-tungstenite) for receiving
- REST API via reqwest for sending
- Config: server IDs, channel IDs to watch
- Implement Channel trait
- Remove relay bot dependency after validation

**Gate:** Send message in watched Discord channel → Nova responds on Telegram within 5s.

### Spec 5: `teams-channel`

**Type:** feature | **Effort:** M | **Deps:** none

Native MS Graph API adapter.
- OAuth2 app registration + token refresh (reqwest + serde)
- Subscription webhooks for channel messages
- REST API for sending replies
- Config: tenant ID, client ID/secret, watched channels
- Implement Channel trait

**Gate:** Message in Teams channel → Nova receives and can respond.

### Spec 6: `imessage-channel`

**Type:** feature | **Effort:** S | **Deps:** none

iMessage via BlueBubbles API.
- HTTP client to BlueBubbles server
- Polling or webhook for new messages
- Send via BlueBubbles REST API
- Config: BlueBubbles server URL, password
- Implement Channel trait

**Gate:** Send iMessage → Nova receives via BlueBubbles → responds on Telegram.

### Spec 7: `email-channel`

**Type:** feature | **Effort:** M | **Deps:** teams-channel (shares MS Graph OAuth)

Email via MS Graph API (Outlook).
- Reuse OAuth token from teams-channel (shared `MsGraphClient`)
- Poll mailbox or webhook subscription
- Filter by sender/subject/folder
- HTML → text extraction for email bodies
- Reply with confirmation
- Implement Channel trait

**Gate:** Receive email → Nova extracts content → responds or notifies via Telegram.

### Spec 8: `jira-webhooks`

**Type:** feature | **Effort:** M | **Deps:** none

Inbound webhook handler for bidirectional Jira sync.
- HTTP endpoint on daemon (add route to existing axum server)
- Parse Jira webhook payloads: issue:updated, issue:created, comment:created
- Update Nova's memory with external changes
- Alert via Telegram when tracked issues change
- Config: webhook secret for validation

**Gate:** Change Jira issue externally → Nova detects and notifies via Telegram.

---

## Wave 4: Hardening — Jira + Callbacks (Fri)

### Spec 9: `harden-jira-integration`

**Type:** task | **Effort:** M | **Deps:** none

Complete the 12 deferred tasks from MVP jira-integration spec.
- Retry wrapper with exponential backoff (429/5xx/network, 3 attempts)
- Callback handlers: approve, edit, cancel
- Expiry sweep: pending actions > 1 hour
- Wire callback routing in agent loop
- HTTP mock tests
- Integration tests (behind env var gate)

**Gate:** `cargo test` passes with new tests. Manual: create issue → edit draft → cancel → verify.

---

## Wave 5: Hardening — Telegram + Nexus (Sat)

### Spec 10: `harden-telegram-nexus`

**Type:** task | **Effort:** S | **Deps:** none

Complete deferred tasks from telegram-channel (2) + nexus-integration (1).
- Telegram integration test (real API, behind env var gate)
- Telegram manual e2e gate
- Wire Nexus error callbacks (`nexus_err:view:`, `nexus_err:bug:`) in Telegram handler

**Gate:** `cargo test` passes. Manual: verify Telegram echo + Nexus error callback.

---

## Wave 6: Architecture — Orchestrator (After Hardening)

### Spec 11: `refactor-orchestrator-pattern`

**Type:** refactor | **Effort:** L | **Deps:** all channels + hardening complete

Replace blocking agent loop with non-blocking orchestrator + worker pool.
- Telegram reactions as read receipts (👀→⏳→✅)
- Worker pool (max 3 concurrent Claude sessions)
- Priority queue (High/Normal/Low)
- Status updates for long-running workers
- Remove thinking message pattern

**Gate:** Send 3 messages rapidly → all get 👀 → responses arrive independently.

---

## Spec Dependency Graph

```
spec-1 (persistent session)
  └─→ spec-4 (discord) — benefits from fast response

spec-2 (diary) ─── independent
spec-3 (voice) ─── independent

spec-4 (discord) ─── independent
spec-5 (teams) ──→ spec-7 (email) — shares MS Graph OAuth
spec-6 (imessage) ── independent
spec-8 (jira webhooks) ── independent

spec-9 (harden jira) ── independent
spec-10 (harden telegram+nexus) ── independent

spec-10 ──→ spec-11 (orchestrator) — depends on all channels + hardening
```

## Wave Execution Plan

| Wave | Day | Specs | Parallelism | Gate |
|------|-----|-------|-------------|------|
| 1 | Mon | persistent-claude-session | Sequential | Response time <3s |
| 2 | Tue | add-interaction-diary, add-voice-reply | Parallel | cargo build + test |
| 3 | Wed-Thu | discord, teams, imessage, email, jira-webhooks | Parallel (5 specs) | Each channel receives + responds |
| 4 | Fri | harden-jira-integration | Sequential | cargo test + manual e2e |
| 5 | Sat | harden-telegram-nexus | Sequential | cargo test + manual e2e |
| 6 | Next | refactor-orchestrator-pattern | Sequential | 3 parallel messages get independent responses |

## Conflict Map

No file conflicts between specs in the same wave:
- Wave 2: diary (diary.rs) vs voice (tts.rs) — different files
- Wave 3: All channels create new module directories — no overlap
- Wave 4-5: Hardening specs touch different files (jira/ vs telegram/ vs nexus/)

The only cross-wave dependency: email-channel reuses MS Graph OAuth from teams-channel.
