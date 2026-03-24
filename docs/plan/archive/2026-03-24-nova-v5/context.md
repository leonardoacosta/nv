# Context: Nova v5

## Previous Phase Summary

Nova v4 (2026-03-23 to 2026-03-24) planned a dashboard/obligation-focused phase but executed a
hardening/tooling phase instead. 36 unplanned specs delivered, 0 of 25 planned specs delivered
under their planned names. See `docs/plan/archive/2026-03-24-nova-v4/COMPLETION.md` for full
retrospective.

## Current Codebase State

- **LOC:** 54,439 (Rust daemon + TypeScript dashboard + Python relay)
- **Tests:** 1,056 (1 pre-existing failure in deploy_watcher)
- **Architecture:** Rust daemon (nv-daemon) with axum HTTP, Telegram/Discord/Teams/iMessage/Email
  channels, 40+ tools, Nexus gRPC client, React SPA dashboard
- **Deployment:** systemd on homelab via git push hook
- **Specs:** 84 archived, 0 open

## Carry-Forward: Deferred Roadmap Items

From nova-v4's undelivered roadmap (prioritized):

### High Priority (core features never started)
1. **Obligation Engine** -- store, detection, alert rules, telegram UX, proactive watchers
   - Was the centerpiece of v4's plan but never started
   - Foundation for proactive behavior
2. **SQLite Versioned Migrations** -- `rusqlite_migration` for messages.db/schedules.db
   - Prerequisite for obligation-store and server-health tables
3. **Server Health Metrics + Crash Detection** -- health snapshots, uptime monitoring
   - Enables self-healing behavior

### Medium Priority (code-aware operations)
4. **Nexus Context Injection** -- "Solve with Nexus" flow from dashboard/Telegram
5. **Nexus Session Progress** -- workflow progress tracking for /apply, /ci:gh
6. **Memory Consistency** -- system prompt reads memory before every response

### Lower Priority (polish)
7. **Dashboard Sidebar Sparkline** -- usage visualization
8. **Tailscale Native Migration** -- Docker to native (may already be done)

## Carry-Forward: Open Beads Epics (3)

| Epic | ID | Children | Status |
|------|----|----------|--------|
| add-service-diagnostics | nv-ekt | 0/21 closed | in_progress |
| fix-nova-amnesia | nv-4u1 | 0/11 closed | open |
| add-voice-reply | nv-53k | 0/14 closed | open |

## Carry-Forward: Known Bugs

- **nv-9vt** (P1): fix-jql-limit-syntax -- JQL query limit syntax error
- **deploy_watcher test**: `watcher_cycle_stores_obligation_for_deploy_failure_rule` panics on
  missing `obligations` table -- needs test setup fix

## Carry-Forward: Open Ideas (25)

Dashboard ideas (8): notifications, mobile-responsive, charts/trends, activity-feed,
approval-queue, conversation-threads, message-history, authentication, websocket-feed

Agent ideas (5): error-recovery-ux, tool-result-caching, proactive-followups,
agent-persona-switching, cc-native-nova

Communication ideas (4): conversation-persistence, callback-handler-completion,
cross-channel-routing, interaction-diary

Media ideas (2): voice-to-text-stt, voice-tts-reply

## Open Questions

1. Should obligation engine remain the top priority, or has the operational landscape changed?
2. Is the 25-spec-per-phase roadmap size realistic? v4 showed 36 reactive specs dominated.
3. Should v5 focus on hardening existing features (tests, reliability) vs new capabilities?
4. The add-service-diagnostics epic (nv-ekt) has 21 children -- complete it or re-scope?
