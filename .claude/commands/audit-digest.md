---
name: audit:digest
description: Audit the digest/briefing system — gather, synthesize, format, schedule
type: command
execution: foreground
---

# Audit: Digest

Audit the scheduled digest and morning briefing system.

## Scope

| Module | File | Purpose |
|--------|------|---------|
| Gather | `crates/nv-daemon/src/digest/gather.rs` | Collect alerts, obligations, Nexus sessions |
| Synthesize | `crates/nv-daemon/src/digest/synthesize.rs` | AI summarization via Claude |
| Format | `crates/nv-daemon/src/digest/format.rs` | HTML/plain text templating |
| Actions | `crates/nv-daemon/src/digest/actions.rs` | Suggested action determination |
| State | `crates/nv-daemon/src/digest/state.rs` | DigestStateManager (hash, suppression) |
| Scheduler | `crates/nv-daemon/src/scheduler.rs` | Cron triggers (digest interval, morning briefing, user schedules) |

## Routes

| # | Method | Path | What to check |
|---|--------|------|---------------|
| 1 | POST | `/digest` | Trigger immediate digest, response format |

## Audit Checklist

### Gather
- [ ] Data sources covered (obligations, Nexus sessions, alerts, recent messages)
- [ ] Graceful handling when sources are unavailable
- [ ] Time window filtering (only recent items)

### Synthesize
- [ ] Claude prompt quality (concise, actionable output)
- [ ] Token budget management (prevent expensive digests)
- [ ] Fallback when Claude API unavailable

### Format
- [ ] HTML output for email (valid, renders correctly)
- [ ] Plain text output for Telegram (Markdown formatting)
- [ ] Empty digest handling (suppress or send minimal?)

### Actions
- [ ] Suggested actions are actionable and specific
- [ ] Priority ordering of suggestions
- [ ] Deduplication with previous digest actions

### State
- [ ] Content hash comparison (suppress identical digests)
- [ ] `last-digest.json` persistence and recovery
- [ ] `actions_suggested` vs `actions_taken` tracking

### Scheduler
- [ ] Digest interval calculation and initial delay on restart
- [ ] Morning briefing fires at 7am local time (timezone handling)
- [ ] User schedule polling (60s interval)
- [ ] Cron event emission correctness

## Memory

Persist findings to: `.claude/audit/memory/digest-memory.md`

## Findings

Log to: `~/.claude/scripts/state/nv-audit-findings.jsonl`
