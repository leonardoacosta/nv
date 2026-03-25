# Nova v8 -- Context

## Previous Phase: nova-v7 (2026-03-25)

Nova v7 delivered 26 specs in a single session:
- Extracted dashboard to Next.js 15 at `apps/dashboard/`
- Added Anthropic Messages API client with SSE streaming
- Switched to native tool_use protocol (from prose augmentation)
- Added SQLite-backed conversation persistence with TTL
- Replaced Nexus gRPC with TeamAgentDispatcher + CcSessionManager
- Added pipeline latency profiling and parallel context build

Completion: `docs/plan/archive/2026-03-25-nova-v7/COMPLETION.md`

## Carry-Forward: Deferred Tasks

### From response-latency-optimization
- Streaming response delivery (Telegram "..." placeholder + progressive edits)
- Persistent subprocess investigation (CC stream-json hang diagnosis)
- Full streaming ClaudeClient pipeline

### From migrate-nova-brain
- Integration test: daemon + dashboard URL configured, verify forwarding

### Manual Smoke Tests (29 across all specs)
- All specs have user manual smoke tests deferred

## Carry-Forward: Open Ideas (17)

| Slug | ID | Category |
|------|-----|----------|
| contact-profiles-system | nv-0bxt | Data/UX |
| ms-graph-cli-tools | nv-rctm | Integrations |
| error-recovery-ux | nv-dkv | UX |
| tool-result-caching | nv-4pq | Performance |
| proactive-followups | nv-7xc | Autonomy |
| agent-persona-switching | nv-n2l | UX |
| dashboard-authentication | nv-x3m | Security |
| callback-handler-completion | nv-l2e | Telegram UX |
| cross-channel-routing | nv-2e6 | Architecture |
| interaction-diary | nv-7n4 | Observability |
| voice-to-text-stt | nv-dnq | Channels |
| voice-tts-reply | nv-4an | Channels |
| telegram-streaming-styled-buttons | nv-32o | Telegram UX |
| telegram-bot-presence | nv-mkp | Telegram UX |
| telegram-reminder-ux | nv-b9t | Telegram UX |
| proactive-obligation-research | nv-lvm | Autonomy |
| self-improvement-research | nv-ad8 | Meta |

## Current Codebase State

- Rust: ~77,855 LOC across nv-daemon + nv-core + nv-cli
- TypeScript: ~9,385 LOC in apps/dashboard/
- SQLite stores: messages, obligations, schedules, server_health, cold_start_events, latency_spans, briefings, conversations
- Dashboard: Next.js 15, 12 pages, cosmic theme, WebSocket events, mobile layout
- Agent management: TeamAgentDispatcher + CcSessionManager (Nexus fully removed)
- API paths: cold-start CLI, persistent session, direct Anthropic HTTP, dashboard forwarding

## Open Questions

1. Dashboard auth (nv-x3m) -- Tailscale-only for now, but should we add auth before expanding access?
2. Streaming delivery -- was partially scoped in v7 but deferred. Priority for v8?
3. Voice channels (STT/TTS) -- two ideas queued, how do they fit with current architecture?
4. Contact system (nv-0bxt) -- new idea, would require schema + migration + all ingestion paths
5. MS Graph tools (nv-rctm) -- auth model mismatch between delegated (CLI) and app (daemon)
