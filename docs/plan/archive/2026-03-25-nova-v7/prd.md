# PRD -- Nova v7

> Lean PRD for engineering-focused phase. Derived directly from scope-lock.md.
> No user stories, financials, or brand artifacts needed.

## Summary

Migrate Nova's intelligence from cold-start Claude CLI subprocesses into a persistent CC session
managed by a Next.js dashboard. Reduce Telegram response latency from 18-30s to under 10s.
Ship all 9 backlog ideas across 4 waves. Deprecate Nexus in favor of CC team agents.

## Architecture

See scope-lock.md § Architecture Direction for current vs target diagrams.

## Spec Pipeline

### Wave 1: Memory & Quick Fixes (current architecture)

| # | Spec | Type | Complexity | Idea Source |
|---|------|------|-----------|-------------|
| 1 | fix-cold-start-memory | bug | small | Architecture gap finding |
| 2 | fix-telegram-null-callback | bug | trivial | nv-zsr |
| 3 | enrich-diary-narratives | feature | small | nv-de2 |
| 4 | improve-typing-indicators | feature | small | nv-b4i |
| 5 | fix-nexus-session-dedup | bug | small | nv-unw |
| 6 | investigate-300s-timeout | bug | small-medium | nv-yhu |
| 7 | fix-reminders-db-migration | bug | trivial | Runtime error |

### Wave 2: Dashboard & Architecture Migration

| # | Spec | Type | Complexity | Idea Source |
|---|------|------|-----------|-------------|
| 1 | extract-nextjs-dashboard | feature | large | nv-4zs + scope decision |
| 2 | cc-session-management | feature | large | Scope decision |
| 3 | migrate-nova-brain | feature | large | Scope decision |
| 4 | add-morning-briefing-page | feature | medium | nv-837 |
| 5 | add-cold-start-logging | feature | small | nv-clp |
| 6 | add-session-slug-names | feature | small | nv-wqd |
| 7 | rebuild-dashboard-wireframes | feature | large | nv-4zs |

### Wave 3: Direct API Fallback (conditional)

| # | Spec | Type | Complexity |
|---|------|------|-----------|
| 1 | add-anthropic-api-client | feature | medium |
| 2 | native-tool-use-protocol | feature | medium |
| 3 | persistent-conversation-state | feature | medium |
| 4 | response-latency-optimization | feature | small |

### Wave 4: Nexus Deprecation

| # | Spec | Type | Complexity |
|---|------|------|-----------|
| 1 | replace-nexus-with-team-agents | feature | large |
| 2 | remove-nexus-crate | refactor | medium |
| 3 | update-session-lifecycle | feature | medium |
| 4 | cleanup-nexus-config | refactor | trivial |

## Success Criteria

- Telegram response latency under 10s (P95)
- Nova remembers its own recent messages without tool calls
- Dashboard matches approved wireframes
- Morning briefing page generates daily at 7am CT
- Zero duplicate Nexus sessions on batch approvals
- All 9 backlog ideas shipped
