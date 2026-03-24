# Plan Completion: nova-v4

## Phase: v4 -- Dashboard, Obligations, and Code-Aware Operations

## Completed: 2026-03-24

## Duration: 2026-03-23 to 2026-03-24 (2 days)

## Delivered (Planned)

0 of 25 planned specs were delivered under their planned names. The phase pivoted from the
planned dashboard/obligation focus to hardening, tooling, and infrastructure work driven by
real-world operational needs discovered during audits.

Partial delivery of planned work:
- Dashboard scaffold exists and is served via rust-embed (`add-dashboard-scaffold` equivalent)
- Dashboard API endpoints exist (`add-dashboard-api` equivalent)
- 6 dashboard pages exist with functional UI
- Nova brand mark is integrated (`add-dashboard-nova-mark` equivalent)
- Proactive watchers exist (`add-proactive-watchers` partially equivalent)

## Delivered (Unplanned)

36 specs delivered, categorized:

**Tool Integrations (12):** ADO, Calendar, Cloudflare DNS, Cron self-management,
Doppler, GitHub deeper, Neon management, Teams Graph, Web fetch, Cross-channel routing,
Deploy hooks, Photo/audio receiving

**Hardening & Bug Fixes (12):** Agent cold-start, Channel safety, Dashboard contracts,
Infra health, Nexus stability, Persistent subprocess, Prompt bloat, Tool result strip,
Tools registry, Watcher reliability, Hardening v3, JQL default project

**Infrastructure (7):** Service diagnostics, Multi-instance services, Nexus session watchdog,
Tool emoji indicators, Tool logging, Secrets migration to Doppler, Nexus proto sync

**Features (5):** Reminders system, Test ping endpoint, Mobile-friendly formatters,
Digest pipeline wiring, HA service call wiring

## Deferred

### Undelivered Roadmap (carry to nova-v5)
- Obligation engine (store, detection, alert rules, telegram UX) -- core planned feature
- SQLite versioned migrations
- Nexus context injection ("Solve with Nexus" flow)
- Nexus session progress tracking
- Server health metrics + crash detection
- Memory consistency (system prompt injection)
- Dashboard sidebar sparkline

### Open Beads Epics (3)
- nv-ekt: add-service-diagnostics (0/21 children closed) -- in_progress
- nv-4u1: fix-nova-amnesia (0/11 children closed) -- open
- nv-53k: add-voice-reply (0/14 children closed) -- open

### Open Beads Tasks: 48 non-idea, 25 ideas

### Known Bug
- nv-9vt: fix-jql-limit-syntax (P1)
- 1 pre-existing test failure: deploy_watcher obligations table missing

## Metrics

- LOC: 54,439 (Rust + TypeScript + Python)
- Tests: 1,056 (1,012 nv-daemon + 37 nv-core + 7 other)
- Specs archived: 60 fully complete (24 with open tasks restored to changes/)
- Beads: 135 total (60 closed, 74 open, 1 in-progress)

## Lessons

### What Worked
- Granular specs with clear tasks.md enable high completion rates (most specs 100%)
- Agent-based execution (/apply) works well for isolated specs
- Parallel agent dispatch effective for independent specs (channel-safety + dashboard-contracts)
- Audit-driven spec creation catches real bugs (6 fix-* specs from domain audits)
- Bulk archiving keeps openspec/changes/ clean

### What Didn't
- Roadmap-execution divergence: 0/25 planned specs delivered -- operational needs dominated
- The obligation engine was never started despite being the core planned feature
- Dashboard pages were scaffolded but contract mismatches went unnoticed until audit
- Test coverage for the deploy_watcher is broken (missing obligations table setup)

### Recommendations for nova-v5
- Start with obligation engine -- it was planned for v4 and deferred
- Add integration tests that validate API contract alignment
- Fix the deploy_watcher test (needs obligations table in test setup)
- Consider whether 25-spec roadmaps are realistic for 2-day phases
- Prioritize nv-9vt (P1 JQL bug) early
