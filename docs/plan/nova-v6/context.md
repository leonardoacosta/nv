# Context: Nova v6

## Previous Phase Summary

Nova v5 completed 2026-03-24. All 4 success gates passed:
- 94 specs archived (34 planned + 60 unplanned)
- P1 JQL bug fixed, amnesia solved, obligation engine end-to-end
- 1,032 tests, ~55K Rust LOC, ~4K TypeScript LOC

Full details: `docs/plan/archive/2026-03-24-nova-v5/COMPLETION.md`

## Primary Goal: CC-Native Nova -- Phase 2 (MCP Extraction)

Migrate Nova's 60+ tool implementations from nv-daemon's monolithic `execute_tool()` dispatch
into a standalone MCP server binary. This is Phase 2 of the 4-phase cc-native-nova migration
(beads nv-k86).

### Architecture: Current vs Target

```
CURRENT (nv-daemon monolith):
  Telegram polling -> orchestrator -> worker -> claude subprocess
                                       |
                                       v
                              tools/mod.rs execute_tool()
                              (60+ tools, 140KB file)

TARGET (Phase 2 complete):
  Telegram polling -> orchestrator -> worker -> claude subprocess
                                       |              |
                                       v              v
                              nv-daemon tools    nv-tools MCP server
                              (wired internally)  (same tools, MCP protocol)
```

### What Phase 2 Delivers
- `nv-tools` binary: standalone MCP server exposing all 60+ tools
- Tool implementations extracted from tools/*.rs into MCP-compatible handlers
- nv-daemon can optionally delegate to MCP server OR use tools directly
- Foundation for Phase 3 (CC session replaces daemon) and Phase 4 (daemon deprecated)

### What Phase 2 Does NOT Do
- Does NOT remove tools from nv-daemon (dual-mode: internal + MCP)
- Does NOT touch Telegram/channel handling (that's Phase 1/3)
- Does NOT require CC Channels to be stable (independent of research preview)

## Carry-Forward: Deferred Tasks

### From jira-integration (7 deferred)
- Retry wrapper with exponential backoff
- Callback handlers (edit, cancel, expiry sweep)
- Integration tests (need mock HTTP server)

### From other specs (3 deferred)
- Nexus error callback wiring
- Orchestrator status_update Telegram message
- BotFather command registration

### Manual Tests (~25 [user] tasks)
- Telegram tool verification across 15+ integrations
- Dashboard visual verification (2 tasks)

## Carry-Forward: Open Ideas (25)

| Slug | ID | Category |
|------|----|----------|
| cc-native-nova | nv-k86 | Architecture (PRIMARY) |
| voice-tts-reply | nv-4an | Voice |
| voice-to-text-stt | nv-dnq | Voice |
| error-recovery-ux | nv-dkv | Agent Intelligence |
| tool-result-caching | nv-4pq | Agent Intelligence |
| proactive-followups | nv-7xc | Agent Intelligence |
| agent-persona-switching | nv-n2l | Agent Intelligence |
| self-improvement-research | nv-ad8 | Agent Intelligence |
| conversation-persistence | nv-93d | Data |
| callback-handler-completion | nv-l2e | Data |
| interaction-diary | nv-7n4 | Data |
| cross-channel-routing | nv-2e6 | Channels |
| telegram-streaming-styled-buttons | nv-32o | Telegram UX |
| telegram-bot-presence | nv-mkp | Telegram UX |
| telegram-reminder-ux | nv-b9t | Telegram UX |
| proactive-obligation-research | nv-lvm | Proactive |
| dashboard-websocket-feed | nv-42e | Dashboard |
| dashboard-authentication | nv-x3m | Dashboard |
| dashboard-message-history | nv-jea | Dashboard |
| dashboard-conversation-threads | nv-9p9 | Dashboard |
| dashboard-approval-queue | nv-8y9 | Dashboard |
| dashboard-activity-feed | nv-e34 | Dashboard |
| dashboard-charts-trends | nv-967 | Dashboard |
| dashboard-notifications | nv-tft | Dashboard |
| dashboard-mobile-responsive | nv-0n8 | Dashboard |

## Codebase State

- Rust workspace: nv-core, nv-daemon, nv-cli (3 crates)
- tools/mod.rs: 140KB, 60+ tools in flat .rs files under tools/
- Dashboard: React SPA with health cards, sparklines, server metrics
- Deploy: systemd on homelab via git push hook
- Data: SQLite (messages, obligations, server_health, schedules, tool_usage)
- Secrets: Doppler for API keys

## Open Questions

1. Should nv-tools be a separate crate in the workspace or a standalone repo?
2. Which MCP transport: stdio (simplest) or HTTP/SSE (supports remote)?
3. Should tools be grouped by domain (jira-mcp, ha-mcp) or one mega-server?
4. What's the testing strategy for MCP tools vs the current unit tests?
