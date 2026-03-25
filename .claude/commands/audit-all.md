---
name: audit:all
description: Run all 7 domain audits for nv with parallel agent dispatch
type: command
execution: foreground
---

# Audit All — nv

Run all 8 domain audits with parallel agent dispatch, progressive collection, and summary report.

## Project

- **Name:** nv
- **Path:** `/home/nyaptor/nv`
- **Type:** Rust workspace (3 crates + React dashboard)
- **Domains:** 7

## Domains

| Domain    | Agent Model | Key Modules                                | Command            |
| --------- | ----------- | ------------------------------------------ | ------------------ |
| agent     | single      | orchestrator, worker, claude, conversation | `/audit:agent`     |
| channels  | single      | telegram, discord, teams, email, imessage  | `/audit:channels`  |
| tools     | single      | 20 service integrations                    | `/audit:tools`     |
| dashboard | single      | React SPA + axum API (11 endpoints)        | `/audit:dashboard` |
| digest    | single      | gather, synthesize, format, scheduler      | `/audit:digest`    |
| watchers  | single      | 4 watchers, alert rules, obligations       | `/audit:watchers`  |
| infra     | single      | health, CLI, config, memory, state, deploy | `/audit:infra`     |

## Execution Plan

### Wave 1 — Core (parallel)

- agent (orchestrator, worker pool, Claude API)
- channels (5 messaging platforms)
- tools (20 service integrations)

### Wave 2 — Features (parallel)

- dashboard (React SPA + API)
- digest (briefing system)
- watchers (proactive monitoring)

### Wave 3 — Infrastructure (parallel)

- infra (health, CLI, config, deploy)

## Agent Dispatch

For each wave, spawn agents in parallel:

```
Wave N:
  Agent 1 → /audit:{domain1}
  Agent 2 → /audit:{domain2}
  Agent 3 → /audit:{domain3}
  ─── Wait for all agents ───
  Collect findings
  ─── Next wave ───
```

## Findings Collection

All agents log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`

After all waves, generate summary:

```
NV Audit Summary
================
Domain      | High | Med | Low | Total
------------|------|-----|-----|------
agent       |    ? |   ? |   ? |     ?
channels    |    ? |   ? |   ? |     ?
...         |    ? |   ? |   ? |     ?
------------|------|-----|-----|------
TOTAL       |    ? |   ? |   ? |     ?
```

## Memory

Each domain persists findings to `.claude/audit/memory/{domain}-memory.md`.
