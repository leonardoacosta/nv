# Roadmap -- Nova v5

> Generated from scope-lock.md. 33 specs across 12 waves, 6 phases.
> Execution order: Debt -> Bugs -> Amnesia -> Obligations -> Tools -> Monitoring

---

## Phase 1: Spec Debt Clearance (Waves 1-4)

24 restored specs with ~55 open tasks. Grouped by task type to maximize parallelism.

### Wave 1: Code Tasks

Specs with implementation work remaining. Conflict: sentry + stripe both init in main.rs.

| Spec | Open Tasks | Files |
|------|-----------|-------|
| `add-sentry-tools` | 3 (init client, audit log, [user]) | main.rs, sentry.rs |
| `add-stripe-tools` | 3 (init client, audit log, [user]) | main.rs, stripe.rs |
| `add-neon-tools` | 2 (audit log, [user]) | neon.rs |
| `rewrite-mobile-friendly-formatters` | 4 (build, clippy, test, [user]) | formatters across tools/ |

**Strategy:** Apply sentry + stripe sequentially (main.rs conflict), neon + formatters in parallel.

### Wave 2: Deferred Implementation

Specs with `[deferred]` code tasks requiring significant implementation.

| Spec | Open Tasks | Work |
|------|-----------|------|
| `jira-integration-mvp-original` | 12 (5 deferred + 7 other) | retry wrapper, callback handlers, expiry sweep |
| `refactor-orchestrator-pattern` | 3 (1 deferred + 2 [user]) | status_update Telegram message |
| `nexus-integration-mvp-original` | 1 (deferred) | Nexus event callback wiring |
| `fix-chat-bugs` | 3 (all deferred) | JSON parse retry tests (need process mocking) |

**Strategy:** Jira is the big one (12 tasks). Others are small. All touch different modules.

### Wave 3: Integration Tests

| Spec | Open Tasks | Work |
|------|-----------|------|
| `telegram-channel-mvp-original` | 2 (integration test + manual) | Telegram API integration test |
| `harden-telegram-nexus` | 3 (integration test + 2 manual) | Telegram + Nexus integration test |

**Strategy:** Both touch Telegram but different test files. Run in parallel.

### Wave 4: User Verification

15 specs with only `[user]` manual test tasks. Leo batch-tests via Telegram in one session.

| Spec | Task |
|------|------|
| `add-ado-list-projects` | "What ADO projects do we have?" |
| `add-ado-tools` | "What pipelines are on ProjectX?" |
| `add-aggregation-layer` | "How's OO?" / "Homelab status" / "Financial summary" |
| `add-bootstrap-soul` | Delete bootstrap state, restart, verify conversation |
| `add-github-tools` | "List PRs on nyaptor/nv" |
| `add-ha-tools` | "Living room temperature?" / "Turn off office lights" |
| `add-neon-management-tools` | "What Neon projects do I have?" |
| `add-plaid-tools` | "What are my account balances?" / verify no PII in audit |
| `add-posthog-tools` | "How many signups on OO this week?" |
| `add-resend-tools` | "Any email bounces?" |
| `add-upstash-tools` | "How's Redis doing?" |
| `add-vercel-tools` | "Latest deploy on otaku-odyssey?" |
| `add-telegram-commands` | Register commands in BotFather |
| `harden-jira-integration` | Full Jira create/edit/cancel/expire flow |

**Strategy:** One sitting. Leo sends messages, verifies responses, marks tasks done.

---

## Phase 2: Bug Fixes (Wave 5)

### Wave 5: P1 Bug + Test Fix

| Spec | Type | Work |
|------|------|------|
| `fix-jql-limit-syntax` | **NEW** (from nv-9vt) | Fix JQL query limit syntax error |
| `fix-deploy-watcher-test` | **NEW** | Fix missing obligations table in test setup |

**Strategy:** Independent files. Parallel.

---

## Phase 3: Amnesia + Memory (Wave 6)

### Wave 6: Conversation Persistence

| Spec | Type | Work |
|------|------|------|
| `fix-conversation-amnesia` | **NEW** (from nv-4u1 epic, 11 beads children) | ConversationStore, identity/user config, wiring |

**Beads children to implement:**
- `nv-xw9`: Create conversation.rs with session expiry and bounds
- `nv-am6`: Register ConversationStore in SharedDeps
- `nv-vra`: Wire into Worker::run (load prior turns + push completed)
- `nv-2un`: tool_result truncation (>1000 chars)
- `nv-37e`: Bump format_recent_for_context 500->2000 + turn-pair grouping
- `nv-56z`: Move history constants from agent.rs
- `nv-4ym`: Populate config/identity.md
- `nv-flo`: Populate config/user.md
- `nv-4eq`: Unit tests: ConversationStore push/load/expire/trim
- `nv-wcd`: Unit test: tool_result truncation
- `nv-20g`: Unit test: format_recent_for_context

**Gate:** Nova references yesterday's conversation context in today's first interaction.

---

## Phase 4: Proactive Behavior (Waves 7-10)

Sequential dependency chain: migrations -> store -> detection -> UX.

### Wave 7: SQLite Migrations

| Spec | Type | Work |
|------|------|------|
| `add-sqlite-migrations` | **NEW** | Add rusqlite_migration, PRAGMA user_version, convert existing tables to v1 |

**Files:** Cargo.toml, messages.rs, reminders.rs, tools/schedule.rs
**Gate:** `PRAGMA user_version` returns 1 after daemon start.

### Wave 8: Obligation Store (depends on Wave 7)

| Spec | Type | Work |
|------|------|------|
| `add-obligation-store` | **NEW** | obligations table via migration, Obligation types, CRUD operations |

**Files:** messages.rs (migration v2), new obligation_store.rs, nv-core/types.rs
**Gate:** Unit tests for CRUD + status transitions.

### Wave 9: Obligation Detection (depends on Wave 8)

| Spec | Type | Work |
|------|------|------|
| `add-obligation-detection` | **NEW** | Claude classification pipeline, obligation creation, priority routing |

**Files:** orchestrator.rs, new obligation_detector.rs, worker.rs
**Gate:** Discord message "can you update X?" creates obligation within 5 minutes.

### Wave 10: Obligation Telegram UX (depends on Wave 9)

| Spec | Type | Work |
|------|------|------|
| `add-obligation-telegram-ux` | **NEW** | Formatted cards, inline keyboard [Handle/Delegate/Dismiss], morning digest |

**Files:** channels/telegram/client.rs, orchestrator.rs
**Gate:** Obligation notification appears in Telegram with inline keyboard.

---

## Phase 5: Tools Reliability (Wave 11)

### Wave 11: Service Diagnostics

| Spec | Type | Work |
|------|------|------|
| `complete-service-diagnostics` | **NEW** (from nv-ekt epic, 21 beads children) | Checkable trait, ServiceRegistry, nv check CLI |

**Beads children include:** Checkable trait, ServiceRegistry, tools/channels restructure,
nv check CLI, health endpoint extension, unit + integration tests.

**Gate:** `nv check --json` returns pass/fail for all 14 service clients.

---

## Phase 6: Monitoring (Wave 12)

### Wave 12: Health Metrics + Dashboard (depends on Wave 7)

| Spec | Type | Work |
|------|------|------|
| `add-server-health-metrics` | **NEW** | server_health table via migration, health poll loop, API endpoint |
| `improve-dashboard-monitoring` | **NEW** | Dashboard health cards, sparklines, real-time status |

**Files:** messages.rs (migration v3), health_poller.rs, http.rs, dashboard/src/
**Gate:** Dashboard shows server metrics with 7-day mini chart data.

---

## Spec Dependency Graph

```
Wave 1-4: Spec Debt (24 existing specs, independent)
  |
  v
Wave 5: Bug Fixes (fix-jql-limit, fix-deploy-watcher-test)
  |
  v
Wave 6: Conversation Amnesia (fix-conversation-amnesia)
  |
  v
Wave 7: SQLite Migrations (add-sqlite-migrations)
  |-> Wave 8: Obligation Store -> Wave 9: Detection -> Wave 10: Telegram UX
  |-> Wave 12: Health Metrics + Dashboard Monitoring
  |
Wave 11: Service Diagnostics (independent of Waves 7-10)
```

## Wave Execution Plan

| Wave | Phase | Specs | Strategy |
|------|-------|-------|----------|
| 1 | Debt | sentry-tools, stripe-tools, neon-tools, mobile-formatters | Sequential (main.rs conflict) then parallel |
| 2 | Debt | jira-mvp, orchestrator, nexus-mvp, chat-bugs | Parallel (different modules) |
| 3 | Debt | telegram-mvp, telegram-nexus | Parallel (different test files) |
| 4 | Debt | 15 specs [user] manual tests | Leo batch session |
| 5 | Bugs | fix-jql-limit, fix-deploy-watcher-test | Parallel |
| 6 | Amnesia | fix-conversation-amnesia | Solo (touches agent.rs, worker.rs, main.rs) |
| 7 | Proactive | add-sqlite-migrations | Solo (foundation) |
| 8 | Proactive | add-obligation-store | Solo (depends on 7) |
| 9 | Proactive | add-obligation-detection | Solo (depends on 8) |
| 10 | Proactive | add-obligation-telegram-ux | Solo (depends on 9) |
| 11 | Tools | complete-service-diagnostics | Solo (large epic, 21 tasks) |
| 12 | Monitoring | server-health-metrics, dashboard-monitoring | Parallel |

**Total: 24 existing + 9 new = 33 specs across 12 waves, 6 phases**

## New Specs to Create

| Spec | Phase | Depends On |
|------|-------|------------|
| `fix-jql-limit-syntax` | 2 | none |
| `fix-deploy-watcher-test` | 2 | none |
| `fix-conversation-amnesia` | 3 | none |
| `add-sqlite-migrations` | 4 | none |
| `add-obligation-store` | 4 | add-sqlite-migrations |
| `add-obligation-detection` | 4 | add-obligation-store |
| `add-obligation-telegram-ux` | 4 | add-obligation-detection |
| `complete-service-diagnostics` | 5 | none |
| `add-server-health-metrics` | 6 | add-sqlite-migrations |
| `improve-dashboard-monitoring` | 6 | add-server-health-metrics |

## Conflict Map

| File | Specs |
|------|-------|
| `main.rs` | add-sentry-tools, add-stripe-tools, fix-conversation-amnesia, complete-service-diagnostics |
| `messages.rs` | add-sqlite-migrations, add-obligation-store, add-server-health-metrics |
| `orchestrator.rs` | add-obligation-detection, add-obligation-telegram-ux |
| `worker.rs` | fix-conversation-amnesia, refactor-orchestrator-pattern |
| `http.rs` | add-server-health-metrics, improve-dashboard-monitoring |
| `agent.rs` | fix-conversation-amnesia |
| `channels/telegram/client.rs` | add-obligation-telegram-ux |

Conflicts resolved by wave ordering -- conflicting specs never run in the same wave.
