---
name: audit:dashboard
description: Audit the web dashboard — React SPA + axum API endpoints
type: command
execution: foreground
---

# Audit: Dashboard

Audit the embedded React dashboard and its backing axum API endpoints.

## Scope

### Backend API (`crates/nv-daemon/src/dashboard.rs`)

| # | Method | Path | Handler | What to check |
|---|--------|------|---------|---------------|
| 1 | GET | `/api/obligations` | `get_obligations` | List with status/owner filters, empty state |
| 2 | PATCH | `/api/obligations/{id}` | `patch_obligation` | Status updates, validation, 404 handling |
| 3 | GET | `/api/projects` | `get_projects` | Project code listing |
| 4 | GET | `/api/sessions` | `get_sessions` | Nexus sessions with progress |
| 5 | POST | `/api/solve` | `post_solve` | Start Nexus session, error injection |
| 6 | GET | `/api/memory` | `get_memory` | Memory topic listing, file reads |
| 7 | PUT | `/api/memory` | `put_memory` | Memory file writes |
| 8 | GET | `/api/config` | `get_config` | Config with secret redaction |
| 9 | PUT | `/api/config` | `put_config` | Config updates (rewrite config.toml) |
| 10 | GET | `/api/server-health` | `get_server_health` | Uptime, channels, CPU/mem/disk |

### SPA Routes

| # | Path | Component | What to check |
|---|------|-----------|---------------|
| 1 | `/` | `DashboardPage` | Overview data loading, layout |
| 2 | `/obligations` | `ObligationsPage` | CRUD operations, status transitions |
| 3 | `/projects` | `ProjectsPage` | Project listing, navigation |
| 4 | `/nexus` | `NexusPage` | Session cards, progress display |
| 5 | `/integrations` | `IntegrationsPage` | Service status cards |
| 6 | `/usage` | `UsagePage` | Cost charts, budget tracking |
| 7 | `/memory` | `MemoryPage` | Memory file viewer/editor |
| 8 | `/settings` | `SettingsPage` | Configuration UI, save flow |

### Frontend (`dashboard/src/`)

- Framework: React + React Router + Vite
- Styling: Tailwind CSS (cosmic-gradient theme, dark mode)
- Key components: Sidebar, ServerHealth, UsageSparkline, ActiveSession, IntegrationCard, NovaMark

## Audit Checklist

### API Layer
- [ ] Secret redaction in `GET /api/config` (no API keys leaked)
- [ ] `PUT /api/config` validation (prevent invalid TOML writes)
- [ ] `PUT /api/memory` path traversal protection
- [ ] Obligation status transitions (valid state machine)
- [ ] Error responses (consistent format, proper HTTP status codes)
- [ ] CORS configuration (if dashboard served separately in dev)

### SPA
- [ ] Static file serving from embedded `dashboard/dist/`
- [ ] Client-side routing fallback (all non-API paths → index.html)
- [ ] Asset hashing for cache busting
- [ ] Build output size and bundle analysis

### Frontend
- [ ] Error states on API failures
- [ ] Loading states during data fetching
- [ ] Empty states (no obligations, no sessions, etc.)
- [ ] Responsive layout
- [ ] Dark mode consistency

## Memory

Persist findings to: `.claude/audit/memory/dashboard-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
