# Context: Nova v4

## Previous Phase Summary

Nova v3 delivered 74 specs in 2 days (24 planned + 50 unplanned), building out the complete
tool ecosystem, multi-channel support, and infrastructure foundation. The daemon runs on
homelab via systemd with Doppler secrets management.

## Current State

### Architecture
- **3-crate workspace**: nv-core (types/config), nv-daemon (channels/tools/orchestrator), nv-cli
- **~50,164 LOC** of Rust, 961 tests passing
- **98 tools** registered (via Nexus proto sync)
- **5 channels**: Telegram (primary), Discord, Teams, Email, iMessage
- **Secrets**: Doppler project `nova/prd` (33 secrets)
- **Deploy**: systemd user services with `doppler run --fallback=true`

### Known Issues
- Nexus `homelab` hostname doesn't resolve — Tailscale runs containerized, MagicDNS unavailable to host. Fix: migrate Tailscale from Docker to native (explored, safe, not yet executed).
- 2 pre-existing test failures in orchestrator module
- 48 open beads issues from v3 work
- Node core dump from sequential-thinking MCP server (libuv abort)

### Runtime
- Daemon: active (running) via systemd
- Health: http://localhost:8400/health
- Channels: Telegram + Discord connected, Nexus disconnected (DNS)
- Tools: All initialized (Stripe, Vercel, Sentry, Doppler, HA, ADO, Cloudflare, etc.)

## Carry-Forward Work

### From v3 (48 open beads)
These are non-spec issues tracked in beads — feature requests, minor improvements, and
infrastructure tasks that accumulated during v3. Review with `bd ready` to prioritize.

### Infrastructure Debt
- Tailscale Docker → native migration (explored, 8 steps, zero blast radius)
- Pre-existing test failures (orchestrator::estimate_complexity_digest_cron)
- Sequential-thinking MCP server stability (node core dumps)

### Opportunities
- **Proactive intelligence**: Nova has all the data sources but is mostly reactive. V4 could add proactive monitoring, anomaly detection, and automated responses.
- **Multi-agent orchestration**: Nexus is connected but underutilized. V4 could add cross-agent task delegation and coordination.
- **Web dashboard**: NV currently has no web UI — all interaction is via Telegram/CLI. A lightweight dashboard could surface tool stats, pending actions, and system health.
- **E2E testing**: No E2E test suite exists. A test harness for the Telegram flow would catch regressions.
- **Plugin architecture**: Tool handlers are hardcoded in mod.rs. A plugin system could allow dynamic tool loading.

## Open Questions
1. Should v4 focus on depth (improving existing tools) or breadth (new capabilities)?
2. Is the multi-agent Nexus vision worth investing in, or should Nova stay single-agent?
3. Should we build a web dashboard, or is Telegram sufficient as the primary interface?
4. What's the priority: reliability/hardening vs new features?
