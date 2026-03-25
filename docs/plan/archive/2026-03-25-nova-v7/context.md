# Nova v7 Context

## Previous Phase: Nova v6 (MCP Extraction)

Completed 2026-03-25. Extracted 45 tools from nv-daemon into standalone nv-tools MCP server.
SharedDeps trait defined. 6 specs delivered, all planned. Tagged: `nova-v6-complete`.

### Post-v6 Session Fixes (2026-03-25, shipped directly)

4 commits landed between v6 and v7 planning:

1. **General-purpose system tools** -- `run_command`, `read_file`, `write_file`, `grep_files`,
   `list_dir` added to daemon tool registry with production deny-list and RTK integration
2. **RTK routing for bash toolkit** -- All scoped commands (git, ls, cat, bd) routed through
   RTK for 60-90% token savings
3. **Tool definitions in system prompt** -- Fixed critical bug where cold-start mode never
   embedded tool definitions. Claude couldn't see any of the daemon's 100+ tools. Now 49KB
   system prompt with all schemas.
4. **Typing indicator fix** -- Removed "..." message edit pattern that spammed Telegram with
   429 rate limit errors. Replaced with single fire-and-forget `sendChatAction("typing")`.

### Infrastructure Changes (not in git)

- Pre-push deploy hook fixed for v6 crates
- Telegram bot token rotated in Doppler `nova/prd`
- systemd MemoryMax bumped 2G -> 4G (OOM from concurrent cold-start CLI subprocesses)
- Global obligation-check hook guarded with existence check
- CC project settings.json with permissive allows + production deny-list

## Deferred from v6

- tailscale.rs daemon coupling (aggregation.rs dependency)
- DaemonSharedDeps concrete implementation
- Jira retry wrapper + callback handlers (7 tasks)
- Nexus error callback wiring
- BotFather command registration
- ~25 manual Telegram verification tasks

## Current State

- Rust LOC: ~61K across 3 crates
- Tests: 36 passing, 1 failing (nv-core config::tests::secrets_from_env_missing_key_is_none)
- Persistent session mode: DISABLED (hardcoded `fallback_only: true` in claude.rs:484)
- Every Telegram message = cold-start Claude CLI subprocess (~18-30s per turn)
- reminders.db migration error (v7 migrations in DB, v6 code -- DatabaseTooFarAhead)

## Architecture Gap: Cold-Start Memory Loss

**Critical finding from this session.** The daemon's cold-start mode has a memory gap:

1. `ConversationStore` is in-memory only, 10min timeout, cleared on restart
2. Digests run inline in the orchestrator, bypass ConversationStore entirely
3. `build_conversation_prompt()` sends only messages to Claude, system prompt has tools
4. Each cold-start is effectively amnesiac -- Nova doesn't remember its own messages

**Recommended fix:** Auto-inject last 5-10 outbound messages from `MessageStore` (SQLite)
into the system prompt. ~20 lines of code, zero extra latency.

## Carry-Forward: Open Ideas (9)

| Slug | ID | Theme |
|------|-----|-------|
| session-diary-narrative | nv-de2 | Memory -- enrich diary entries with narrative summaries |
| morning-briefing-digest | nv-837 | Memory -- daily AI-written briefing page on dashboard |
| cold-start-dashboard-logging | nv-clp | Observability -- surface cold-start events to dashboard |
| dashboard-wireframe-drift | nv-4zs | Dashboard -- realign UI to approved wireframes |
| telegram-null-bubble-on-approve | nv-zsr | Telegram UX -- fix null callback answer on button press |
| session-slug-names-with-dashboard-links | nv-wqd | Telegram UX -- human-readable session names with links |
| request-timeout-300s-investigation | nv-yhu | Reliability -- trace and mitigate 300s timeout |
| telegram-typing-and-presence-status | nv-b4i | Telegram UX -- richer typing indicators per phase |
| nexus-duplicate-sessions-vs-team-agents | nv-unw | Reliability -- dedup sessions or pivot to team agents |

## Scope Direction

Nova v7 is a broad UX + reliability + memory phase. Three pillars:

### Pillar 1: Memory & Context (highest impact)
- Fix cold-start memory loss (inject recent outbound messages)
- Session diary narrative summaries
- Morning briefing digest on dashboard
- Cold-start event logging

### Pillar 2: Telegram UX
- Null bubble on approve/edit/cancel
- Session slug names with dashboard links
- Richer typing indicators
- Timeout handling and feedback

### Pillar 3: Reliability
- Nexus duplicate session prevention
- 300s timeout investigation
- reminders.db migration fix
- Persistent session mode investigation (CC CLI bug status)

## Open Questions

1. Should persistent session mode be re-investigated? CC CLI has had updates since the bug.
2. Is the 49KB system prompt (100 tools) sustainable? Could reduce by only sending relevant tools per context.
3. Should the dashboard be rebuilt from wireframes, or incrementally fixed?
4. Team agents vs Nexus -- which direction for multi-project coordination?
