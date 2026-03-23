# Scope Lock — Nova v4

## Vision
Nova v4 transforms from a reactive tool-querying daemon into a proactive personal operations
system that watches, remembers, and acts — surfacing obligations Leo didn't know he had,
across every channel and system he touches.

## Target Users
- **Leo** — Lead DevOps/SWE, LLC operator, 20+ projects across work and personal. Primary
  interaction via Telegram (mobile) and web dashboard (desktop).

## Domain
Nova is a personal operations daemon running on homelab. It integrates with Leo's professional
tools (Jira, ADO, Teams, Outlook, GitHub, Vercel, Sentry, Stripe) and personal systems
(Home Assistant, Discord, iMessage, Plaid, Google Calendar) to provide unified visibility,
proactive alerting, and cross-channel obligation tracking.

**In scope for v4:**
- Proactive intelligence (deploy watching, error spikes, stale tickets, HA anomalies)
- Cross-channel obligation detection ("someone asked for X" → tracked automatically)
- Web dashboard (axum API + React SPA, embedded via rust-embed)
- Session/network stability hardening (Claude CLI resilience, reconnection, memory consistency)
- Multi-agent Nexus maturation (reliable connections, cross-agent task delegation)
- Code-aware operations (Nexus-powered code changes, performance analysis, codebase research via Claude Code sessions)
- SQLite migration infrastructure (rusqlite_migration for schema evolution)
- Tailscale Docker → native migration

**Out of scope for v4:**
- Multi-tenant / multi-user support
- Plugin/dynamic tool loading architecture
- Mobile native app
- E2E test harness (deferred to v5)

## Differentiator
Unlike Grafana/Datadog (monitoring dashboards), Nova is an **agent** that understands context
across channels. It doesn't just show metrics — it detects obligations, tracks follow-through,
and proactively surfaces "you said you'd do X and haven't."

Unlike generic AI assistants, Nova is deeply integrated with Leo's actual infrastructure —
it can query any system, cross-reference data, and take actions with confirmation.

## Features to Steal
- **Linear's auto-triage**: Automatically categorize and prioritize incoming requests
- **Reclaim.ai's commitment detection**: Parse natural language for action items
- **Grafana alerting rules**: Threshold-based alerts with configurable conditions

## v4 Must-Do
Two pillars:
1. **Proactive obligation detection**: Nova watches all channels and surfaces "things Leo needs
   to respond to or act on" — without being asked.
2. **Code-aware operations**: Nova can analyze codebases, identify pain points, research
   solutions, and delegate code changes to Nexus agents — bridging the gap between "ops
   daemon that reads data" and "engineering partner that takes action."

## v4 Won't-Do
- NLP model training/fine-tuning (use Claude for text understanding)
- Calendar scheduling (read-only integration is enough)
- Payment processing or financial automation
- CI/CD pipeline changes (Nova monitors, doesn't modify)

## Business Model
Personal tool. No revenue. Clean enough to open-source later (no hardcoded secrets in code,
configurable via TOML + Doppler).

## Brand Direction
- **Aesthetic**: Dark, minimal, data-dense. Think terminal-inspired with subtle color coding.
- **Personality**: Direct, competent, slightly opinionated. Not chatty, not corporate.
- **Dashboard**: Status-board feel — glanceable cards, time-series charts, obligation queue.
  Desktop-first with responsive mobile (read-heavy on mobile, full control on desktop).

## Scale Target
- 1 user (Leo)
- ~20 projects monitored
- ~5 channels active
- ~100 tools registered
- Dashboard: <100ms page loads (embedded, local network)

## Hard Constraints
- Homelab deployment only (no cloud hosting)
- All secrets via Doppler (no flat files)
- Single binary preferred (dashboard embedded via rust-embed)
- SQLite for persistence (no Postgres dependency for NV itself)
- Rust for daemon, React (Vite) for dashboard SPA
- No breaking changes to existing Telegram/CLI workflows

## Timeline
No external pressure. Quality over speed. Expect organic scope expansion (v3 was 3x planned).

## Assumptions Corrected
- **"Nova is mostly reactive"** → v4's primary goal is making Nova proactive. Obligation
  detection, deploy monitoring, and anomaly alerting are all proactive capabilities.
- **"Dashboard needs T3 Turbo"** → Research showed NV already has axum at :8400. Dashboard is
  axum JSON API + embedded React SPA (rust-embed). No Node runtime needed in production.
- **"Commitment tracking = NLP"** → Broader than commitments. It's cross-channel obligation
  detection — client requests on Discord, manager asks on Teams, customer emails. Claude
  handles the text understanding; Nova provides the ingestion, storage, and surfacing.
- **"SQLite migrations aren't needed"** → They are. `CREATE TABLE IF NOT EXISTS` breaks on
  `ALTER TABLE`. `rusqlite_migration` must be added before any schema changes.
- **"Memory loss is a feature gap"** → It's a reliability bug. Nova has memory files but
  doesn't consistently read them before responding. Fix is in the system prompt and
  session management, not new infrastructure.
