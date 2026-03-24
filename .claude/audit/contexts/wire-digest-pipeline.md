# Context: Wire Digest Pipeline Into Runtime

## Source: Audit 2026-03-23 (digest domain, graded F — entire pipeline dead code)

## Problem
The digest module (gather, synthesize, format, actions, state) is fully implemented and tested but NEVER called from the runtime. All modules carry `#[allow(dead_code)]`. The orchestrator's Digest branch just sends "Digest triggered" to Claude generically.

## Decision Required
Wire the pipeline into the orchestrator Digest branch, replacing the generic passthrough.

## Findings

### P1 — Pipeline is dead code
- `crates/nv-daemon/src/digest/mod.rs:1` — all sub-modules have #[allow(dead_code)]
- `gather_context`, `synthesize_digest`, `format_digest`, `should_send`, `record_sent` are never called from orchestrator/worker/handler
- What actually happens: CronEvent::Digest → TriggerClass::Digest → generic worker → Claude gets string "[cron] Digest triggered"

### P2 — No Jira result limit or date filter
- `crates/nv-daemon/src/digest/gather.rs:137`
- Fetches all open unresolved Jira issues with no count limit or date filter
- Large backlogs produce unbounded prompt size

### P2 — No max_tokens enforced for digest Claude call
- `crates/nv-daemon/src/digest/synthesize.rs:59`
- Only text instruction to Claude about length — no max_tokens parameter

### P2 — inject_budget_warning() never called
- `crates/nv-daemon/src/digest/synthesize.rs:94`
- Defined but never wired into any caller

### P3 — Morning briefing fires at any hour >= 7
- `crates/nv-daemon/src/scheduler.rs:111`
- Condition: `current_hour >= MORNING_BRIEFING_HOUR`
- If daemon restarts at 9pm with stale last_briefing_date, briefing fires at 9pm
- Fix: Narrow to `current_hour == MORNING_BRIEFING_HOUR`

### P3 — dismiss_all_actions() does N load+save cycles
- `crates/nv-daemon/src/digest/actions.rs:21`
- Calls update_action_status() per pending action — N reads + N writes
- Fix: Load once, mutate all, save once

### P2 — cmd_digest() hardcodes port 8400
- `crates/nv-daemon/src/orchestrator.rs:1009` (shared with agent domain)
- Fix: Read configured port from SharedDeps/config

## Implementation Plan
1. Remove #[allow(dead_code)] from digest modules
2. In orchestrator's Digest branch: call gather_context → synthesize_digest → format_digest
3. Use state.should_send() to suppress identical digests
4. Wire inject_budget_warning() into synthesize call
5. Add .limit(20) to Jira JQL in gather
6. Add max_tokens to synthesize Claude call
7. Fix morning briefing hour check

## Files to Modify
- `crates/nv-daemon/src/orchestrator.rs` (Digest branch)
- `crates/nv-daemon/src/digest/mod.rs` (remove dead_code attrs)
- `crates/nv-daemon/src/digest/gather.rs` (Jira limit)
- `crates/nv-daemon/src/digest/synthesize.rs` (max_tokens, budget warning)
- `crates/nv-daemon/src/digest/actions.rs` (batch save)
- `crates/nv-daemon/src/scheduler.rs` (morning briefing)
