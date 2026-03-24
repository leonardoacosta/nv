# Digest Domain Audit Memory

**Audited:** 2026-03-23  
**Files:** gather.rs, synthesize.rs, format.rs, actions.rs, state.rs, scheduler.rs  
**Auditor:** codebase-health-analyst

---

## Critical Finding: Pipeline Is Not Wired Into Runtime

The entire `digest/` module (gather, synthesize, format, actions, state) is dead code in production. The `CronEvent::Digest` trigger is dispatched to the generic worker pool, which formats it as `"[cron] Digest triggered"` and sends it to Claude via the standard agent loop. The structured pipeline (gather context from Jira/Nexus/Memory/Calendar → synthesize via Claude → format for Telegram → track state) is never invoked.

**Evidence:**
- `digest/mod.rs` has `#[allow(dead_code)]` on every sub-module
- `gather_context`, `synthesize_digest`, `format_digest`, `should_send`, `record_sent` are only referenced within the digest module itself (tests + internal helpers)
- The orchestrator's `TriggerClass::Digest` arm falls through to `worker dispatch` without calling any digest function
- `inject_budget_warning()` is defined but never called

**Decision needed:** Either wire the pipeline into the orchestrator's Digest branch, or remove the module and rely solely on the generic agent loop.

---

## Checklist Results

### Gather
- [PASS] Parallel gather with independent 30s timeouts per source
- [PASS] Graceful degradation when sources unavailable (partial results accepted)
- [FAIL] No time-window filtering on Jira issues (fetches ALL open issues, no limit/date cap)
- [FAIL] No time-window filtering on memory (all topics, unbounded)
- [PASS] Calendar scoped to today via `gather_today_for_digest()`
- [N/A] Recent messages — not a data source in this pipeline

### Synthesize
- [PASS] Claude prompt is concise and actionable with clear section structure
- [CONCERN] No API-level token budget enforcement (text asks <3000 chars but no max_tokens)
- [PASS] Fallback `synthesize_digest_fallback()` exists for Claude unavailability
- [FAIL] `inject_budget_warning()` exists but is never called

### Format
- [PASS] Telegram plain text output with 4096-char truncation and line preservation
- [FAIL] UTF-8 unsafe truncation — `&text[..budget]` can panic on non-ASCII char boundary
- [FAIL] No HTML formatter for email (checklist item unmet)
- [PASS] Empty digest handling (returns no keyboard when no actions)

### Actions
- [CONCERN] DigestActionType always hardcoded to `FollowUpQuery` regardless of action content
- [CONCERN] Section detection is fragile (plain string contains, not anchored pattern)
- [PASS] Maximum 5 actions enforced
- [PASS] Deduplication implicit — state tracks actions per-digest by ID

### State
- [PASS] SHA-256 content hash for suppression of identical digests
- [PASS] `last-digest.json` with atomic write (tmp + rename)
- [PASS] Graceful recovery when file missing or empty (`{}`  → Default)
- [CONCERN] `dismiss_all_actions()` does N load+save cycles instead of one batched save

### Scheduler
- [PASS] Initial delay calculated from last digest state (no spurious fire on restart)
- [PASS] Morning briefing fires at 7am via `current_hour >= MORNING_BRIEFING_HOUR` with daily dedup via `last_briefing_date`
- [CONCERN] Morning briefing can fire at any hour >= 7 (e.g., 9pm on daemon restart), not strictly at 7am
- [PASS] User schedule polling at 60s interval
- [PASS] Cron events emitted correctly; scheduler exits cleanly on channel close
- [CONCERN] `mark_run()` called while Mutex guard is held (minor contention risk)

### Route: POST /digest
- [PASS] Returns 202 Accepted immediately (async dispatch)
- [PASS] Returns 503 when agent loop is not running
- [CONCERN] `cmd_digest()` in orchestrator hardcodes port 8400 instead of using configured port

---

## Severity Summary

| Severity | Count | Key Issues |
|----------|-------|------------|
| High | 2 | Pipeline entirely dead code; `#[allow(dead_code)]` suppressing warnings |
| Medium | 4 | No Jira/memory time-window limits; no API token budget; port hardcoded |
| Low | 5 | UTF-8 unsafe truncation; N saves in dismiss_all; fragile action parser; morning briefing timing; Mutex contention |

---

## Recommended Actions

1. **P1** — Decide pipeline fate: wire `gather_context` → `synthesize_digest` → `format_digest` into the orchestrator's Digest branch, or delete the module and remove `#[allow(dead_code)]` from `mod.rs`
2. **P2** — Add `.limit(20)` to Jira JQL and cap memory topic count in `gather_memory()`
3. **P2** — Add `max_tokens` to the `synthesize_digest` Claude call (e.g., 1024)
4. **P2** — Fix `cmd_digest()` to use the configured port from config/state rather than hardcoded 8400
5. **P3** — Fix UTF-8 truncation in `truncate_for_telegram`
6. **P3** — Batch `dismiss_all_actions()` into a single save
7. **P3** — Pin morning briefing to fire only within the 7:00–7:59 window (not any hour >= 7)
