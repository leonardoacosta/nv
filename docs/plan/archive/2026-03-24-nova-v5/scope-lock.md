# Scope Lock -- Nova v5

## Vision

Nova v5 clears accumulated spec debt, solves the amnesia problem, and lays the foundation for
proactive behavior through an obligation engine backed by versioned SQLite migrations.

## Target Users

Leo (sole user) -- software engineer who needs Nova to remember context across sessions, detect
obligations from inbound messages, and surface tool failures without manual checking.

## Domain

**In scope:**
- Batch-apply 24 restored specs (~55 open tasks, mostly tests/verification)
- Fix P1 bug nv-9vt (JQL limit syntax)
- Fix pre-existing deploy_watcher test failure
- Memory system: solve amnesia (nv-4u1 epic, 11 tasks)
- SQLite versioned migrations (rusqlite_migration, PRAGMA user_version)
- Obligation engine: store, detection, alert rules, Telegram notification
- Service diagnostics completion (nv-ekt epic, 21 tasks)
- Dashboard contract alignment and monitoring improvements

**Out of scope:**
- Voice reply (nv-53k epic) -- defer to v6
- Dashboard authentication -- single-user homelab, no auth needed
- Dashboard mobile responsive -- desktop-only is fine
- Multi-user support -- this is a personal tool
- Cloud deployment -- homelab only
- New channel integrations -- 5 channels is enough

## Priority Order

| Phase | Focus | Success Criteria |
|-------|-------|-----------------|
| 1 | Spec debt clearance | 24 restored specs complete, P1 bug fixed, deploy_watcher test fixed |
| 2 | Amnesia + Memory | nv-4u1 epic complete, Nova references prior context in conversation |
| 3 | Proactive behavior | SQLite migrations, obligation store, detection, Telegram notification |
| 4 | Tools reliability | nv-ekt service diagnostics epic complete, tool failures surfaced |
| 5 | Monitoring | Dashboard pages reflect real API contracts, health metrics visible |

## Success Gates (v5 is "done" when)

1. All 24 restored specs have 0 open tasks
2. nv-9vt P1 JQL bug is fixed
3. Nova remembers and references prior session context (amnesia solved)
4. Obligation detection works end-to-end (message -> classify -> store -> notify)

Gates 1-3 are hard requirements. Gate 4 is the stretch goal.

## DB Strategy

Add `rusqlite_migration` crate for versioned migrations with `PRAGMA user_version`. Convert
existing `CREATE TABLE IF NOT EXISTS` patterns to migration v1. All new tables (obligations,
server_health) added as subsequent migrations.

## Planning Model

Everything in the roadmap is priority work. Unplanned additions are expected -- they represent
real operational needs discovered during execution. Additions will be documented in the
reconciliation section at the next `/plan:advance`.

## Hard Constraints

- Deployment: systemd on homelab via git push hook
- Runtime: single Rust binary (nv-daemon) + React SPA dashboard
- Data: all local SQLite, no cloud dependencies for core function
- Secrets: Doppler for API keys, local env for machine-specific values

## Timeline

No external deadline. Self-paced development. Phases are ordered by priority, not calendar.

## Assumptions Corrected

- v4 planned 25 specs and delivered 0 by name -> v5 accepts that additions happen alongside
  planned work; reconciliation at phase end captures the full picture
- "84 archived specs" was incorrect -> 24 were prematurely archived with open tasks; now restored
- Dashboard pages exist but have contract mismatches -> fixed in v4, monitoring improvements in v5
- Obligation engine was "never started" -> still the highest-leverage unbuilt feature after amnesia
