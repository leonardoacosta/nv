# Nova v9 -- Context

## Previous Phase: nova-v8 (2026-03-26)

Nova v8 delivered 30 specs in a single session:
- 18 planned features (Telegram UX, voice, contacts, MS Graph, autonomy, polish)
- 6 dashboard remediation specs (API proxy, content rendering, Geist redesign)
- 6 final fixes (port mismatch, stubs, empty states, autonomy, visibility, Agent SDK sidecar)

Completion: `docs/plan/archive/2026-03-26-nova-v8/COMPLETION.md`

## Critical Carry-Forward

### P0: Agent SDK Sidecar Not Functional
The sidecar spawns but crashes because `claude-agent-sdk` Python package isn't installed.
System has no pip. Need to bootstrap pip first:
```bash
curl -sS https://bootstrap.pypa.io/get-pip.py | python3
pip install claude-agent-sdk
systemctl --user restart nv.service
```
This is the ONLY blocker for Nova's tool access.

### P0: ANTHROPIC_API_KEY Invalid
The key in Doppler is an OAuth token (sk-ant-oat), not an API key.
The Agent SDK sidecar bypasses this — it uses OAuth natively via CC CLI.
Once the sidecar works, this is no longer an issue.

## Carry-Forward: Open Ideas (1)

| Slug | ID | Category |
|------|-----|----------|
| dashboard-authentication | nv-x3m | Security |

## Current Architecture

```
Telegram → nv-daemon (Rust, port 8400)
             ├── Worker pool (3 concurrent)
             ├── Agent SDK sidecar (Python, stdin/stdout)
             │     └── MCP tools → POST /api/tool-call → daemon
             ├── Obligation executor (autonomous, idle-only)
             ├── Proactive watcher (30min, 7am-10pm)
             ├── Digest (7am daily with morning briefing)
             └── HTTP API for dashboard

apps/dashboard (Next.js 15, Docker, Traefik)
  ├── 16 pages (Geist design system)
  ├── WebSocket activity feed
  ├── Obligation management (stats, cards, CRUD)
  └── Proxies to daemon /api/*
```

## Open Questions

1. pip/package management — should we use a venv, uv, or system pip for the sidecar?
2. Agent SDK vs raw API — once sidecar works, should we keep AnthropicClient as fallback?
3. Dashboard auth — Tailscale-only for now, when to add proper auth?
4. Streaming — disabled when using AnthropicClient/sidecar, how to restore?
5. Test coverage — 152 specs delivered but zero automated E2E tests
