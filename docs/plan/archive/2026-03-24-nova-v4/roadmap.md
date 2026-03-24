# Roadmap -- Nova v4

> Generated from v4 PRD. ~30 specs across 5 phases, 8 waves.
> Execution order: Infra Foundation -> Obligation Engine -> Dashboard -> Code-Aware Ops -> Polish

---

## Wave 1: Infrastructure Foundation (Day 1)

### Spec 1: `add-sqlite-migrations`

**Type:** infra | **Effort:** S | **Deps:** none

Add `rusqlite_migration` to both SQLite databases (messages.db, schedules.db). Implement
versioned migrations with `PRAGMA user_version`. Convert existing `CREATE TABLE IF NOT EXISTS`
patterns to migration v1. All future schema changes go through migrations.

**Files:** `Cargo.toml`, `messages.rs`, `reminders.rs`, `tools/schedule.rs`
**Gate:** `PRAGMA user_version` returns 1 after daemon start.

### Spec 2: `migrate-tailscale-native`

**Type:** infra | **Effort:** S | **Deps:** none

Move Tailscale from Docker container to native `tailscaled` on the host. Copy state from
Docker volume, install native package, enable systemd service, verify MagicDNS resolves
`homelab` and `macbook` hostnames. Update `compose/vpn.yml` to remove tailscale service.

**Files:** homelab project (not NV codebase), nv.toml (host field stays `homelab`)
**Gate:** `getent hosts homelab` resolves. Nexus agents connect.

### Spec 3: `harden-session-stability`

**Type:** bugfix | **Effort:** M | **Deps:** none

Fix Claude CLI session management: retry on malformed JSON (once, then cold-start fallback),
configurable session timeout, graceful error reporting to user channel. Fix memory consistency:
system prompt reads memory before every response. Fix channel reconnection: exponential backoff
on Telegram/Discord/Teams disconnect.

**Files:** `worker.rs`, `agent.rs`, `claude.rs`, `channels/telegram/mod.rs`,
`channels/discord/gateway.rs`, system-prompt.md
**Gate:** Daemon runs 7 days without manual restart.

---

## Wave 2: Obligation Engine -- Schema and Detection (Day 2 AM)

### Spec 4: `add-obligation-store`

**Type:** feature | **Effort:** M | **Deps:** add-sqlite-migrations

Create `obligations` table via migration. Rust types: `Obligation`, `ObligationStatus`,
`ObligationOwner`. CRUD operations in a new `obligation_store.rs` module. Query methods:
list by status, list by owner, count open, mark acknowledged/handled/dismissed.

**Files:** `messages.rs` (migration), new `obligation_store.rs`, `nv-core/types.rs`
**Gate:** Unit tests for CRUD + status transitions.

### Spec 5: `add-obligation-detection`

**Type:** feature | **Effort:** L | **Deps:** add-obligation-store

Obligation detection pipeline: after every inbound message, call Claude to classify
(is this an action request? is Leo responsible? what project? what priority?). Store
detected obligations. Classify as NOVA CAN HANDLE or LEO MUST HANDLE with reasoning.
Send Telegram notification for P0-P1. Respect quiet hours.

**Files:** `orchestrator.rs`, new `obligation_detector.rs`, `worker.rs`
**Gate:** Discord message "can you update X?" creates obligation within 5 minutes.

---

## Wave 3: Obligation Engine -- Alert Rules and Proactive Watchers (Day 2 PM)

### Spec 6: `add-alert-rules`

**Type:** feature | **Effort:** M | **Deps:** add-obligation-store

Alert rule system: `alert_rules` table via migration. Rule types: deploy_failure (Vercel
status ERROR), sentry_spike (error count > N in M minutes), stale_ticket (Jira in-progress
> N days), ha_anomaly (entity state duration > threshold). Rules create obligations when
triggered. Configurable via dashboard Settings (later) and nv.toml (now).

**Files:** `messages.rs` (migration), new `alert_rules.rs`, `nv-core/config.rs`
**Gate:** Deploy failure rule triggers obligation creation.

### Spec 7: `add-proactive-watchers`

**Type:** feature | **Effort:** M | **Deps:** add-alert-rules

Cron-triggered watchers that evaluate alert rules: deploy_watcher (polls Vercel every 5m),
sentry_watcher (polls error counts every 10m), stale_ticket_watcher (daily), ha_watcher
(polls HA states every 15m). Each watcher checks relevant rules and creates obligations
on match.

**Files:** `orchestrator.rs` (cron triggers), new `watchers/` module (deploy.rs, sentry.rs,
tickets.rs, ha.rs)
**Gate:** Deploy failure auto-detected and obligation created without user interaction.

---

## Wave 4: Dashboard -- Scaffold and API (Day 3 AM)

### Spec 8: `add-dashboard-scaffold`

**Type:** feature | **Effort:** M | **Deps:** none

Create React SPA with Vite in `dashboard/` directory. Configure `rust-embed` in nv-daemon
to serve built assets from `/`. Axum route: `GET /` serves `index.html`, static assets from
`/assets/*`. Geist Sans + Geist Mono fonts via npm. Tailwind CSS with Nova v4 design tokens.
Sidebar component with 8-page navigation. Router (React Router or TanStack Router).

**Files:** new `dashboard/` directory, `Cargo.toml` (rust-embed dep), `http.rs` (SPA route)
**Gate:** `cargo build` embeds dashboard assets. `http://localhost:8400/` serves the SPA.

### Spec 9: `add-dashboard-api`

**Type:** feature | **Effort:** M | **Deps:** add-obligation-store, add-dashboard-scaffold

REST API endpoints in axum: `/api/obligations` (GET, PATCH), `/api/projects` (GET),
`/api/sessions` (GET), `/api/server-health` (GET), `/api/memory` (GET, PUT),
`/api/config` (GET, PUT). JSON responses. CORS not needed (same-origin embedded).

**Files:** `http.rs` (new routes), possibly new `api/` module
**Gate:** `curl http://localhost:8400/api/obligations` returns JSON array.

---

## Wave 5: Dashboard -- Pages (Day 3 PM)

### Spec 10: `add-dashboard-page-dashboard`

**Type:** feature | **Effort:** M | **Deps:** add-dashboard-api

Dashboard page: recent sessions feed with trigger source (channel icon), Leo involvement
badge, inline service tags, session message. Today summary cards (sessions, obligations,
API cost, tools). Live refresh.

**Files:** `dashboard/src/pages/Dashboard.tsx`, `dashboard/src/components/SessionCard.tsx`

### Spec 11: `add-dashboard-page-obligations`

**Type:** feature | **Effort:** M | **Deps:** add-dashboard-api

Obligations page: split by NOVA CAN HANDLE / LEO MUST HANDLE sections. Each item shows
priority bar, urgency badge, title, source with channel icon, owner reasoning, action
buttons. Handled Today collapsed section. Link to History page.

**Files:** `dashboard/src/pages/Obligations.tsx`, `dashboard/src/pages/ObligationHistory.tsx`

### Spec 12: `add-dashboard-page-projects`

**Type:** feature | **Effort:** M | **Deps:** add-dashboard-api

Projects page: table with provider icon row per project, accordion detail with error list,
"Solve with Nexus" per error, Nova notes (recommended/improvements/accomplished), action
buttons (Start Nexus, Open Vercel/Sentry/Jira).

**Files:** `dashboard/src/pages/Projects.tsx`, `dashboard/src/components/ProjectAccordion.tsx`

### Spec 13: `add-dashboard-page-nexus`

**Type:** feature | **Effort:** M | **Deps:** add-dashboard-api

Nexus page: two-column layout. Left: active sessions with live telemetry (elapsed time,
tool count, last output, progress bar for workflow commands), session history table. Right:
server health cards with metrics (CPU, memory, disk, uptime, crash history).

**Files:** `dashboard/src/pages/Nexus.tsx`, `dashboard/src/components/ServerHealth.tsx`

### Spec 14: `add-dashboard-page-integrations`

**Type:** feature | **Effort:** M | **Deps:** add-dashboard-api

Integrations page: failing/unconfigured bubbled to top. Channel list with status dot, detail,
usage stats, configure button. Tool groups (Developer/CI, Finance/Data, Infrastructure).
Configure modal wireframe for each integration.

**Files:** `dashboard/src/pages/Integrations.tsx`, `dashboard/src/components/IntegrationCard.tsx`

### Spec 15: `add-dashboard-page-usage`

**Type:** feature | **Effort:** S | **Deps:** add-dashboard-api

Usage page: API cost summary (inline, not cards), daily cost chart placeholder, credentials
table, tool usage breakdown table. Time range selector (7d/30d/all).

**Files:** `dashboard/src/pages/Usage.tsx`

### Spec 16: `add-dashboard-page-memory`

**Type:** feature | **Effort:** S | **Deps:** add-dashboard-api

Memory page: two-column file browser. Left: file list with name, size, last modified.
Right: content preview with Edit/Raw buttons. Reads from `/api/memory`.

**Files:** `dashboard/src/pages/Memory.tsx`

### Spec 17: `add-dashboard-page-settings`

**Type:** feature | **Effort:** S | **Deps:** add-dashboard-api

Settings page: editable fields (dropdowns, inputs, toggles) for agent config, quiet hours,
obligation detection settings. System info (read-only). Save button writes to `/api/config`.

**Files:** `dashboard/src/pages/Settings.tsx`

---

## Wave 6: Code-Aware Operations (Day 4 AM)

### Spec 18: `add-nexus-context-injection`

**Type:** feature | **Effort:** M | **Deps:** harden-session-stability

"Solve with Nexus" flow: when triggered from dashboard (per-error) or Telegram, inject
error context (Sentry stack trace, Vercel build log, error file/line) directly into the
Nexus `start_session` prompt as `/openspec:explore` with pre-loaded context.

**Files:** `nexus/client.rs`, `tools/mod.rs`, `http.rs` (new endpoint for solve action)
**Gate:** Dashboard "Solve with Nexus" starts CC session with error context pre-loaded.

### Spec 19: `add-nexus-session-progress`

**Type:** feature | **Effort:** M | **Deps:** add-nexus-context-injection

Track progress for known workflow commands (`/apply`, `/ci:gh --fix`, `/feature`). Parse
Nexus session events to determine phase (DB/API/UI/E2E for `/apply`, triage/investigate
for `/ci:gh`). Expose via `/api/sessions` for dashboard progress bar. Non-workflow sessions
show elapsed time only.

**Files:** `nexus/client.rs`, `nexus/events.rs` (new), `http.rs`
**Gate:** `/apply` session shows accurate phase progress on dashboard.

---

## Wave 7: Server Health and Crash Detection (Day 4 PM)

### Spec 20: `add-server-health-metrics`

**Type:** feature | **Effort:** M | **Deps:** add-sqlite-migrations

`server_health` table via migration. Nexus health endpoint extension: return CPU, memory,
disk, uptime, active session count. Nova polls health every 60s and stores snapshots.
Dashboard Nexus page reads from `/api/server-health`.

**Files:** `messages.rs` (migration), `nexus/client.rs` (health poll), `http.rs`
**Gate:** Dashboard shows server metrics with 7-day mini chart data.

### Spec 21: `add-crash-detection`

**Type:** feature | **Effort:** M | **Deps:** add-server-health-metrics, add-obligation-detection

Detect server crashes: compare uptime between health polls (uptime decrease = restart).
On crash detection, create P1 obligation. Spawn investigation session via Nexus to check
`journalctl` for crash cause. Store crash event in server_health with cause and recommendation.

**Files:** `nexus/client.rs`, `obligation_detector.rs`, `watchers/server.rs` (new)
**Gate:** OOM restart detected, investigation spawned, recommendation stored.

---

## Wave 8: Polish and Hardening (Day 5)

### Spec 22: `add-dashboard-nova-mark`

**Type:** feature | **Effort:** S | **Deps:** add-dashboard-scaffold

Integrate Nova mark SVG (`brand/nova-mark.svg`) into dashboard sidebar at 20px. Use mark
as favicon. Nova identity badge (violet-300 `#c4b5fd`) and Leo identity badge (rose-400
`#fb7185`) as reusable components across all pages.

**Files:** `dashboard/src/components/NovaMark.tsx`, `dashboard/public/favicon.svg`

### Spec 23: `add-obligation-telegram-ux`

**Type:** feature | **Effort:** M | **Deps:** add-obligation-detection

Obligation notifications on Telegram: formatted cards with priority, source, action, and
owner classification. Inline keyboard: [Handle] [Delegate to Nova] [Dismiss]. Morning
briefing digest: "Here's what you missed overnight" with obligation queue summary.

**Files:** `channels/telegram/client.rs`, `orchestrator.rs` (morning digest trigger)
**Gate:** Obligation notification appears in Telegram with inline keyboard.

### Spec 24: `fix-memory-consistency`

**Type:** bugfix | **Effort:** S | **Deps:** none

Update system-prompt.md to explicitly instruct: "Before every response, read your memory
files." Add memory file listing to the system prompt injection. Verify Nova references
prior context in multi-turn conversations.

**Files:** `system-prompt.md`, `agent.rs` (system prompt builder)
**Gate:** Nova references yesterday's conversation in today's first interaction.

### Spec 25: `add-dashboard-sidebar-sparkline`

**Type:** feature | **Effort:** S | **Deps:** add-dashboard-page-usage

Sidebar brand area: compressed bar graph showing session and weekly usage (similar to
`S 20% 1:37h  W 4% 4d` from the CLI statusline). Reads from `/api/stats`.

**Files:** `dashboard/src/components/Sidebar.tsx`, `dashboard/src/components/UsageSparkline.tsx`

---

## Spec Dependency Graph

```
spec-1 (sqlite-migrations)
  |-> spec-4 (obligation-store) -> spec-5 (obligation-detection)
  |                              -> spec-6 (alert-rules) -> spec-7 (proactive-watchers)
  |-> spec-20 (server-health) -> spec-21 (crash-detection)
  |-> spec-9 (dashboard-api, also needs spec-4)

spec-2 (tailscale-native) -- independent

spec-3 (session-stability)
  |-> spec-18 (nexus-context) -> spec-19 (session-progress)

spec-8 (dashboard-scaffold) -- independent
  |-> spec-9 (dashboard-api)
  |-> specs 10-17 (dashboard pages, all need spec-9)
  |-> spec-22 (nova-mark)
  |-> spec-25 (sidebar-sparkline, needs spec-15)

spec-24 (memory-consistency) -- independent
spec-23 (telegram-ux, needs spec-5)
```

## Wave Execution Plan

| Wave | Day | Specs | Strategy |
|------|-----|-------|----------|
| 1 | Day 1 | sqlite-migrations, tailscale-native, session-stability | Parallel (3 independent) |
| 2 | Day 2 AM | obligation-store, obligation-detection | Sequential (store then detection) |
| 3 | Day 2 PM | alert-rules, proactive-watchers | Sequential (rules then watchers) |
| 4 | Day 3 AM | dashboard-scaffold, dashboard-api | Sequential (scaffold then API) |
| 5 | Day 3 PM | 8 dashboard pages (10-17) | Parallel (all pages independent) |
| 6 | Day 4 AM | nexus-context-injection, nexus-session-progress | Sequential |
| 7 | Day 4 PM | server-health-metrics, crash-detection | Sequential |
| 8 | Day 5 | nova-mark, telegram-ux, memory-consistency, sidebar-sparkline | Parallel (polish) |

**Total: 25 specs across 8 waves, 5 phases**

## Conflict Map

| File | Specs |
|------|-------|
| `messages.rs` | sqlite-migrations, obligation-store, alert-rules, server-health |
| `orchestrator.rs` | obligation-detection, proactive-watchers, telegram-ux |
| `http.rs` | dashboard-scaffold, dashboard-api, nexus-context, server-health |
| `nexus/client.rs` | nexus-context, session-progress, server-health, crash-detection |
| `worker.rs` | session-stability |
| `agent.rs` | session-stability, memory-consistency |

These conflicts are resolved by wave ordering -- conflicting specs never run in the same wave.

---

## Execution Reconciliation

**0 of 25 planned specs were delivered under their planned names.** The nova-v4 phase pivoted from
the planned dashboard/obligation focus to hardening, tooling, and infrastructure work driven by
real-world operational needs.

### Planned But Not Delivered (25 specs)

Carry forward to nova-v5 as scope candidates:

- `add-sqlite-migrations` -- versioned migration system for SQLite databases
- `migrate-tailscale-native` -- move Tailscale from Docker to native
- `harden-session-stability` -- session timeout, retry, reconnection
- `add-obligation-store` -- obligations table and CRUD
- `add-obligation-detection` -- Claude-powered obligation classification
- `add-alert-rules` -- deploy failure, sentry spike, stale ticket rules
- `add-proactive-watchers` -- cron-triggered watcher evaluation
- `add-dashboard-scaffold` -- React SPA with Vite (partially delivered -- dashboard exists)
- `add-dashboard-api` -- REST API endpoints (partially delivered -- API exists)
- `add-dashboard-page-dashboard` through `add-dashboard-page-settings` (8 pages -- partially delivered)
- `add-nexus-context-injection` -- "Solve with Nexus" flow
- `add-nexus-session-progress` -- workflow progress tracking
- `add-server-health-metrics` -- health snapshots table
- `add-crash-detection` -- uptime decrease detection
- `add-dashboard-nova-mark` -- brand mark integration (already done)
- `add-obligation-telegram-ux` -- obligation inline keyboard
- `fix-memory-consistency` -- system prompt memory injection
- `add-dashboard-sidebar-sparkline` -- usage sparkline in sidebar

### Unplanned Additions (36 specs delivered)

Specs added mid-phase that were not in the original roadmap:

- `add-ado-list-projects` -- Azure DevOps project listing tool
- `add-calendar-integration` -- Google Calendar read-only tools
- `add-cloudflare-dns-tools` -- Cloudflare DNS zone and record tools
- `add-cron-self-management` -- schedule CRUD tools for self-managed crons
- `add-cross-channel-routing` -- send_to_channel and list_channels tools
- `add-deploy-hooks` -- pre-push/post-merge git hooks for deployment
- `add-doppler-tools` -- Doppler secrets inspection tools
- `add-github-deeper-tools` -- extended GitHub read-only tools
- `add-hardening-v3` -- Jira key validation, JQL limits, tool empty-check
- `add-multi-instance-services` -- generic multi-instance service config
- `add-neon-management-tools` -- Neon REST API tools (projects, branches, compute)
- `add-nexus-session-watchdog` -- background Nexus health monitoring
- `add-photo-audio-receiving` -- Telegram photo/audio reception with vision and STT
- `add-reminders-system` -- user-facing reminder/timer system
- `add-service-diagnostics` -- Checkable trait, ServiceRegistry, nv check CLI
- `add-teams-graph-tools` -- Microsoft Graph API tools for Teams
- `add-test-ping-endpoint` -- GET /test/ping e2e pipeline smoke test
- `add-tool-emoji-indicators` -- real-time emoji tool status in Telegram
- `add-web-fetch-tools` -- fetch_url, search_web, extract_links tools
- `fix-agent-cold-start` -- six cold-start bugs (multi-tool parser, JSON streaming)
- `fix-channel-safety` -- 11 channel defects (UTF-8 panics, Teams clientState, Discord resume)
- `fix-dashboard-contracts` -- 8 API/frontend contract mismatches in dashboard
- `fix-infra-health` -- 11 audit findings (channel status, systemd, deploy hooks)
- `fix-nexus-stability` -- 8 Nexus correctness fixes (double counters, zombie sessions)
- `fix-persistent-claude-subprocess` -- CC v2.1.81 stream-json regressions
- `fix-prompt-bloat` -- stop embedding full system prompt in every Claude call
- `fix-tool-result-strip` -- harden tool artifact cleanup in worker
- `fix-tools-registry` -- 8 tool registry correctness issues
- `fix-watcher-reliability` -- 7 watcher defects (obligation flooding, stale tickers)
- `improve-tool-logging` -- structured tracing at execute_tool entry/exit
- `jira-default-project-fallback` -- fallback to default_project when Claude omits field
- `migrate-secrets-to-doppler` -- replace ~/.nv/env with Doppler
- `rewrite-mobile-friendly-formatters` -- mobile-optimized Telegram formatting
- `sync-nexus-proto` -- align nexus.proto with upstream
- `wire-digest-pipeline` -- connect gather/synthesize/format/actions digest modules
- `wire-ha-service-call` -- connect ha_service_call tool to Home Assistant API

Total: 36 unplanned specs delivered alongside 0 of 25 planned.
