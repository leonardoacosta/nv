# Plan Completion: master-agent-harness (Nova MVP)

## Phase: V1 MVP
## Completed: 2026-03-22
## Duration: 2026-03-21 evening → 2026-03-22 afternoon (~16 hours)

## Delivered

### Original Roadmap (10 specs)
1. cargo-workspace-scaffold — Cargo workspace with 3 crates
2. core-types-and-config — Config, types, Channel trait
3. telegram-channel — Bot API with long-polling + inline keyboards
4. agent-loop — Event-driven Claude CLI subprocess with tool use
5. memory-system — Markdown files + grep search + context injection
6. jira-integration — REST v3 client with confirmation flow
7. proactive-digest — Cron scheduler with notification gating
8. context-query — Cross-system queries with follow-up state
9. nexus-integration — gRPC session awareness + event streaming
10. systemd-deploy — Service file, health endpoint, log rotation

### Beyond Roadmap (same session)
- add-bootstrap-soul — 4-file prompt separation + first-run bootstrap
- add-message-store — SQLite persistence + auto-context injection + `nv stats`
- Claude CLI OAuth subprocess (no API key needed)
- Sandbox isolation (no CLAUDE.md/hooks/MCP leak)
- System prompt v2 (competitive research from 10 competitors)
- Filesystem tools (Read/Glob/Grep/Bash git)
- Discord relay bot + Teams webhook relay
- Markdown-to-HTML converter, thinking ticker, edit-or-fallback delivery
- OOM fix (512M → 2G), Jira v3 search endpoint migration

## Deferred (carry-forward to post-MVP)

### From completed specs (15 tasks)
- jira-integration: retry wrapper, callback handlers, HTTP mocks, integration tests (12)
- telegram-channel: integration test + manual e2e (2)
- nexus-integration: error callback wiring (1)

### Written specs, not applied (23 tasks)
- add-interaction-diary: daily markdown log (9 tasks)
- add-voice-reply: Telegram voice via ElevenLabs TTS (14 tasks)

### Planned, not written (4 specs)
- discord-channel (native — relay exists)
- teams-channel (native — webhook exists)
- imessage-channel
- jira-webhooks (bidirectional sync)

## Metrics
- **LOC:** 13,726 Rust + 191 Python = 13,917 total
- **Tests:** 307 passing (0.51s)
- **Specs:** 11 archived / 14 total (5 open with deferred work)
- **Commits:** 40+ in MVP session
- **Binary:** 476K (nv-daemon release)

## Lessons

### What worked
- Sequential `/apply` with single agent per spec — reliable, simple, no worktree complexity
- OpenClaw-inspired channel trait — clean abstraction for future channels
- System prompt competitive research — stole best patterns from 10 competitors
- File-based config symlinked to repo — editable without recompile
- SQLite message store — solved the context loss problem definitively

### What didn't
- Claude CLI subprocess latency (~8-14s per turn) — architectural limitation
- Markdown parse mode → HTML switch was needed from day one
- OOM at 512M was a surprise (Claude CLI binary is ~500MB)
- Bootstrap conversation needed 3 iterations to get right (too structured → too eager → right)
- `--tools ""` prevented filesystem access; switching to `Read,Glob,Grep,Bash(git:*)` was better

### What should have been done in MVP
- HTML parse mode from start (not Markdown)
- 2G memory limit from start
- Thinking indicator from start (user sees nothing for 10-30s otherwise)
- Message store from start (conversation context was unreliable without it)
