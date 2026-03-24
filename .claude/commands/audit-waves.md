---
name: audit:waves
description: Run domain audits in prioritized waves with filtering support
type: command
execution: foreground
---

# Audit Waves — nv

Run domain audits in prioritized waves. Supports filtering to specific domains.

## Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--domain <name>` | all | Run only specified domain(s), comma-separated |
| `--wave <N>` | all | Run only specified wave number |
| `--dry-run` | off | Show plan without executing |

## Project

- **Name:** nv
- **Path:** `/home/nyaptor/nv`
- **Domains:** 8

## Wave Plan

### Wave 1 — Core Agent Loop
Priority: Highest — these are the critical path for all functionality.

| Domain | Modules | Memory |
|--------|---------|--------|
| agent | orchestrator, worker, claude, conversation | `.claude/audit/memory/agent-memory.md` |
| channels | telegram, discord, teams, email, imessage | `.claude/audit/memory/channels-memory.md` |

### Wave 2 — Service Layer
Priority: High — external integrations and dashboard.

| Domain | Modules | Memory |
|--------|---------|--------|
| tools | 20 service integrations | `.claude/audit/memory/tools-memory.md` |
| dashboard | React SPA + axum API | `.claude/audit/memory/dashboard-memory.md` |

### Wave 3 — Features
Priority: Medium — digest, watchers, nexus.

| Domain | Modules | Memory |
|--------|---------|--------|
| digest | gather, synthesize, format, scheduler | `.claude/audit/memory/digest-memory.md` |
| watchers | 4 watchers, alert rules, obligations | `.claude/audit/memory/watchers-memory.md` |
| nexus | gRPC client, query, notifications | `.claude/audit/memory/nexus-memory.md` |

### Wave 4 — Infrastructure
Priority: Lower — supporting systems.

| Domain | Modules | Memory |
|--------|---------|--------|
| infra | health, CLI, config, memory, state, deploy | `.claude/audit/memory/infra-memory.md` |

## Domain Filter

| Domain | Key Files | Schemas |
|--------|-----------|---------|
| agent | orchestrator.rs, worker.rs, claude.rs, conversation.rs, agent.rs | — |
| channels | channels/*, messages.rs | messages table |
| tools | tools/*.rs, tools/mod.rs (170K) | — |
| dashboard | dashboard.rs, dashboard/src/ | obligations table |
| digest | digest/*.rs, scheduler.rs | last-digest.json |
| watchers | watchers/*.rs, alert_rules.rs, obligation_store.rs | obligations, alert_rules tables |
| nexus | nexus/*.rs, query/*.rs | — |
| infra | health*.rs, config.rs, memory.rs, state.rs, nv-cli/ | messages.db, state/ |

## Execution

For each wave:
1. Spawn domain agents in parallel
2. Wait for completion
3. Collect findings from JSONL
4. Report wave summary
5. Proceed to next wave

Findings: `~/.claude/scripts/state/nv-audit-findings.jsonl`
