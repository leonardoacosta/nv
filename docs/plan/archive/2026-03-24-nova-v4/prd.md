# Product Requirements Document -- Nova v4

## 1. Vision and Problem Statement

Nova v4 transforms from a reactive tool-querying daemon into a proactive personal operations
system that watches, remembers, and acts -- surfacing obligations Leo didn't know he had,
across every channel and system he touches.

**Problem:** Leo manages 20+ projects across work (Jira, ADO, Teams, Outlook, GitHub, Vercel,
Sentry, Stripe) and personal systems (Home Assistant, Discord, iMessage, Plaid, Calendar).
Obligations arrive on any channel at any time -- client requests on Discord, manager asks on
Teams, deploy failures on Vercel. Without proactive detection, obligations fall through cracks.

**Differentiator:** Unlike monitoring dashboards (Grafana/Datadog), Nova is an agent that
understands context across channels. Unlike generic AI assistants, Nova is deeply integrated
with Leo's actual infrastructure and can take confirmed actions.

*Source: scope-lock.md*

## 2. Target Users

### Leo -- Mobile Operator
- Interface: Telegram on iPhone
- Goals: See obligations in <10 seconds, approve/reject with one tap, quick cross-system queries
- Pain: Verbose output, buried confirmation buttons, no "what did I miss?" view

### Leo -- Desktop Commander
- Interface: Web dashboard + CLI
- Goals: Full system health across all projects, drill into errors, delegate code changes via Nexus, manage obligation queue
- Pain: Constant tab-switching between Jira/Vercel/Sentry/GitHub/ADO, no unified view

### Leo -- AFK / Sleeping
- Interface: None (Nova operates autonomously)
- Goals: Watch deploys/errors/channels, queue obligations, P0 alerts still push to Telegram
- Pain: Overnight obligations lost, empty digests, no morning briefing

*Source: scope-lock.md, user-stories.md*

## 3. Success Metrics

| Metric | Target | How Measured |
|--------|--------|-------------|
| Obligations detected | 90%+ of incoming requests across all channels | Manual audit of missed items over 7 days |
| Time to surface | <5 minutes from inbound message to Telegram/dashboard notification | Timestamp delta in obligation store |
| Nova auto-handle rate | 30%+ of obligations resolved without Leo | Obligation history: nova vs leo owner ratio |
| Dashboard page load | <100ms | Embedded SPA on local network, measured via browser devtools |
| Session stability | <1 crash per week | systemd journal: restart count |
| Memory consistency | Nova references prior context in 80%+ of multi-turn conversations | Manual audit of conversation quality |

*Source: scope-lock.md (Scale Target, v4 Must-Do)*

## 4. Functional Requirements

### 4.1 Proactive Obligation Detection

**FR-1:** Nova analyzes every inbound message across all channels (Telegram, Discord, Teams,
Email, iMessage) to determine if it contains an obligation for Leo.

**FR-2:** Obligation analysis uses Claude to classify: (a) is this an action request, (b) is
Leo the responsible party (explicitly or implicitly), (c) what project does it relate to,
(d) what priority level (P0-P3).

**FR-3:** Detected obligations are persisted in SQLite with: source channel, original message
text, detected action, project code, priority, timestamp, status (open/acknowledged/handled/dismissed),
owner (nova/leo).

**FR-4:** Each obligation is classified as NOVA CAN HANDLE (Nova has the tools and context) or
LEO MUST HANDLE (requires human judgment, approval, or domain knowledge), with reasoning.

**FR-5:** Open obligations surface on both Telegram (for P0-P1) and the web dashboard obligation
queue (all priorities). Overdue threshold: 2 hours configurable.

**Acceptance Criteria:**
- Client message on Discord requesting a feature update creates an obligation within 5 minutes
- Manager ask on Teams creates an obligation tagged with the correct project
- Deploy failure detected by cron creates a P0 obligation with "nova can handle" classification
- Obligations persist across daemon restarts (SQLite)

### 4.2 Code-Aware Operations

**FR-6:** Nova can receive a request to investigate a codebase issue (error, performance, pain
point) and delegate investigation to a Nexus agent via `start_session`.

**FR-7:** The "Solve with Nexus" action on the dashboard injects error context (Sentry stack
trace, Vercel build log, file path) directly into the Nexus session prompt via
`/openspec:explore`.

**FR-8:** Nova monitors active Nexus sessions and reports progress for known workflow commands
(`/apply`, `/ci:gh --fix`, `/feature`). Non-workflow sessions show elapsed time only.

**FR-9:** Nova detects server crashes (OOM, kernel panic, service restart) via Nexus health
metrics and proactively investigates the cause, recommending mitigations.

**Acceptance Criteria:**
- "Solve with Nexus" on a Sentry error starts a CC session with the error context pre-loaded
- Session progress bar shows accurate phase for `/apply` commands
- Server crash triggers investigation within 10 minutes of restart detection

### 4.3 Web Dashboard

**FR-10:** Dashboard is a React SPA (Vite) embedded in the nv-daemon binary via `rust-embed`,
served from the existing axum HTTP server at port 8400.

**FR-11:** Dashboard pages (8 total):
1. **Dashboard** -- Recent sessions with trigger source, Leo involvement, inline service tags
2. **Obligations** -- Split by NOVA CAN HANDLE / LEO MUST HANDLE, urgency-sorted
3. **Projects** -- All projects with provider icons, accordion detail, "Solve with Nexus" per error
4. **Nexus** -- Two-column: active sessions with telemetry + server health metrics
5. **Integrations** -- All channels and tools with status, usage stats, configure modals
6. **Usage** -- Claude API costs, credentials, token trends, tool usage breakdown
7. **Memory** -- File browser for Nova's memory files with content preview
8. **Settings** -- Editable agent config, quiet hours, obligation detection settings

**FR-12:** Dashboard sidebar is fixed; main content scrolls independently. Sidebar includes
Nova mark (20px), name, online status, and compressed usage sparkline.

**FR-13:** Dashboard uses Geist Sans for UI text and Geist Mono for data values. Color palette:
cosmic purple (`#7c3aed`) + rose (`#e11d48`) on Vercel black (`#000000`).

**Acceptance Criteria:**
- Dashboard loads in <100ms on local network (no external CDN dependencies at runtime)
- All 8 pages navigate correctly with consistent sidebar state
- Obligation queue updates reflect real-time state (polling or SSE)
- Settings changes persist to `nv.toml` and take effect without daemon restart

### 4.4 Session and Network Stability

**FR-14:** Claude CLI session management handles: malformed JSON (retry once, fallback to
cold-start), session timeout (configurable, default 300s), and graceful error reporting to
the user channel.

**FR-15:** Channel reconnection: Telegram, Discord, and Teams connections automatically
reconnect with exponential backoff on disconnect. Nexus agents reconnect on DNS resolution
(requires Tailscale native migration).

**FR-16:** Memory consistency: Nova reads memory files before every session response. The system
prompt explicitly instructs Claude to check memory before answering.

**FR-17:** Tool failures are logged with structured tracing (tool name, duration, error) and
surfaced to the user if the failure affects the response quality.

**Acceptance Criteria:**
- Daemon runs 7+ days without manual restart
- Channel disconnection recovers within 60 seconds
- Nova references prior conversation context in multi-turn interactions

### 4.5 Infrastructure

**FR-18:** SQLite migration infrastructure using `rusqlite_migration` with `PRAGMA user_version`
tracking. All future schema changes go through versioned migrations.

**FR-19:** Tailscale Docker-to-native migration: move Tailscale from Docker container to native
`tailscaled` on the host. Preserves node identity via state file copy. Enables MagicDNS
resolution for Nexus agent hostnames.

**FR-20:** Nexus server health metrics: CPU, memory, disk, uptime, active sessions, crash
history. Collected via Nexus gRPC health endpoint or system commands.

**Acceptance Criteria:**
- `PRAGMA user_version` increments on each migration
- `tailscale status` resolves `homelab` and `macbook` hostnames from the host
- Nexus health endpoint returns server metrics within 1 second

## 5. Business Case

Personal tool. No revenue model. Value is measured in:
- Time saved context-switching between tools (estimated 30-60 min/day)
- Obligations caught that would have been missed (estimated 2-3/week)
- Code fixes delegated to Nova/Nexus instead of manual investigation

Clean enough to open-source later: configurable via TOML + Doppler, no hardcoded secrets.

*Source: scope-lock.md (Business Model)*

## 6. Design Language

### 6.1 Mark
OR6 v6 "Nova" -- organic constellation with radar sweep. Bright center node, asymmetric star
pattern with curved bezier connections, partial arc, gradient sweep line, rose alert dot.
File: `brand/nova-mark.svg`.

### 6.2 Color System

| Role | Token | Value |
|------|-------|-------|
| Background | `--bg` | `#000000` (Vercel black) |
| Surface | `--surface` | `#0a0a0a` |
| Border | `--border` | `#1a1a1a` |
| Primary brand | `--primary` | `#7c3aed` (violet-600) |
| Nova identity | `--nova` | `#c4b5fd` (violet-300) |
| Leo identity | `--leo` | `#fb7185` (rose-400) |
| Rose/urgency | `--rose` | `#e11d48` (rose-600) |
| Success | `--success` | `#34d399` |
| Warning | `--warning` | `#fbbf24` |
| Danger | `--danger` | `#f87171` |

### 6.3 Typography
- UI: Geist Sans (400/600)
- Data: Geist Mono (400/500)
- Scale: 9px (xs) to 28px (2xl)

### 6.4 Icons
- Brand logos: svgl.app CDN
- Brands not in svgl (Jira): Simple Icons CDN
- UI icons: Phosphor Icons (React)
- Nova mark: custom SVG
- No emoji anywhere

*Source: design.md, brand/nova-mark.svg*

## 7. Technical Architecture

### Stack
- **Daemon:** Rust (nv-core, nv-daemon, nv-cli) -- 50K LOC, 961 tests
- **Dashboard:** React + Vite SPA, embedded via rust-embed into daemon binary
- **API:** axum 0.8 at port 8400 (existing endpoints + new dashboard API)
- **Persistence:** SQLite (messages.db, schedules.db) with rusqlite_migration
- **Secrets:** Doppler (nova/prd, 33 secrets)
- **Deploy:** systemd user services with `doppler run --fallback=true`
- **Agent:** Claude CLI (sonnet-4-6) with tool dispatch

### New API Endpoints (Dashboard)

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/obligations` | GET | List obligations (filterable by status, owner) |
| `/api/obligations/:id` | GET/PATCH | Get or update obligation status |
| `/api/projects` | GET | Project health summary with provider status |
| `/api/sessions` | GET | Active and recent Nexus sessions |
| `/api/server-health` | GET | Nexus server metrics (CPU, memory, disk) |
| `/api/memory` | GET | List memory files |
| `/api/memory/:file` | GET/PUT | Read or update memory file content |
| `/api/config` | GET/PUT | Read or update nv.toml settings |
| `/` | GET | Serve embedded React SPA |

### Data Model (New Tables)

```sql
-- Obligations
CREATE TABLE obligations (
  id TEXT PRIMARY KEY,
  source_channel TEXT NOT NULL,
  source_message TEXT NOT NULL,
  detected_action TEXT NOT NULL,
  project_code TEXT,
  priority INTEGER NOT NULL DEFAULT 2,
  status TEXT NOT NULL DEFAULT 'open',
  owner TEXT NOT NULL DEFAULT 'leo',
  owner_reason TEXT,
  created_at TEXT NOT NULL,
  acknowledged_at TEXT,
  handled_at TEXT,
  dismissed_at TEXT
);

-- Server health snapshots
CREATE TABLE server_health (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  agent_name TEXT NOT NULL,
  cpu_percent REAL,
  memory_used_gb REAL,
  memory_total_gb REAL,
  disk_used_gb REAL,
  disk_total_gb REAL,
  uptime_secs INTEGER,
  active_sessions INTEGER,
  timestamp TEXT NOT NULL
);

-- Alert rules
CREATE TABLE alert_rules (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  trigger_type TEXT NOT NULL,
  trigger_config TEXT NOT NULL,
  projects TEXT,
  channel TEXT NOT NULL DEFAULT 'dashboard',
  enabled INTEGER NOT NULL DEFAULT 1
);
```

*Source: scope-lock.md (Hard Constraints), research from explore session*

## 8. Scope and Constraints

### 8.1 In Scope
- Proactive obligation detection across all 5 channels
- Cross-channel obligation classification (nova/leo ownership)
- Web dashboard (8 pages, embedded SPA)
- Code-aware operations via Nexus
- Session/network stability hardening
- SQLite migration infrastructure
- Tailscale native migration
- Server health monitoring with crash detection

### 8.2 Out of Scope
- Multi-tenant / multi-user support
- Plugin/dynamic tool loading
- Mobile native app
- E2E test harness (v5)
- NLP model training (use Claude)
- Calendar scheduling (read-only)
- Payment processing
- CI/CD pipeline modification (monitor only)

### 8.3 Hard Constraints
- Homelab deployment only
- All secrets via Doppler
- Single binary (dashboard embedded)
- SQLite persistence (no Postgres for NV)
- Rust daemon + React SPA
- No breaking changes to Telegram/CLI workflows

*Source: scope-lock.md*

## 9. Timeline

No external deadline. Quality over speed. Based on v3 experience (74 specs in 2 days for
tool integration), v4's scope is larger (proactive systems, dashboard, stability) but
infrastructure is mature. Estimated 25-40 specs.

Expected organic scope expansion: budget for 2x planned scope based on v3 actuals.
