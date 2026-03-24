# Proposal: Wire Digest Pipeline Into Runtime

## Change ID
`wire-digest-pipeline`

## Summary

The digest module (gather, synthesize, format, actions, state) is fully
implemented but has never been called from the runtime. Every `#[allow(dead_code)]`
attribute in `digest/mod.rs` is a signal: the pipeline is inert. This spec
wires it end-to-end, fixes four production-safety gaps in gather and
synthesize, tightens the morning briefing time window, and replaces the N-cycle
dismiss loop with a single load/mutate/save pass.

## Context
- Extends: `crates/nv-daemon/src/orchestrator.rs` (Digest branch, cmd_digest)
- Extends: `crates/nv-daemon/src/digest/mod.rs` (dead_code attrs)
- Extends: `crates/nv-daemon/src/digest/gather.rs` (Jira query)
- Extends: `crates/nv-daemon/src/digest/synthesize.rs` (max_tokens, budget warning)
- Extends: `crates/nv-daemon/src/digest/actions.rs` (dismiss_all_actions)
- Extends: `crates/nv-daemon/src/scheduler.rs` (morning briefing hour check)
- Extends: `crates/nv-daemon/src/worker.rs` (SharedDeps — add claude_client)
- Source: Audit 2026-03-23 (digest domain, graded F)

## Motivation

The digest pipeline has been built and tested in isolation but is silently
bypassed at runtime. When `CronEvent::Digest` fires, the orchestrator falls
through to the worker pool where Claude receives the literal string
`"[cron] Digest triggered"` — no context, no synthesis, no state recording.
The full gather → synthesize → format → state path is never touched.

Alongside the wiring, four production-safety issues must be resolved before
the pipeline is safe to activate:

1. **Unbounded Jira query** — no result cap means a large backlog produces an
   unbounded prompt. Cap at 20 issues.
2. **No max_tokens on the Claude call** — only a text instruction to stay under
   3000 characters. Add a `max_tokens` parameter to the API call.
3. **inject_budget_warning() never called** — defined in synthesize.rs but has
   no call site. Wire it after synthesis.
4. **cmd_digest() hardcodes port 8400** — uses a literal instead of reading the
   configured `health_port` from `SharedDeps`. Add `health_port: u16` to
   `SharedDeps` and read it in `cmd_digest()`.

Two lower-severity issues also addressed:

5. **Morning briefing fires at any hour >= 7** — if the daemon restarts at
   21:00 with a stale `last_briefing_date`, a briefing fires at 9pm. Narrow
   to `current_hour == MORNING_BRIEFING_HOUR`.
6. **dismiss_all_actions() does N load+save cycles** — one
   `update_action_status()` call per pending action. Replace with a single
   load, in-place mutation, and single save.

## Requirements

### Req-1: Remove dead_code suppression

Remove all five `#[allow(dead_code)]` attributes from `digest/mod.rs`. The
compiler will surface any remaining unreachable paths as errors, which must
be resolved rather than suppressed.

### Req-2: Add ClaudeClient and health_port to SharedDeps

Add two fields to `SharedDeps` in `worker.rs`:

```rust
pub claude_client: ClaudeClient,
pub health_port: u16,
```

Populate both in the `SharedDeps { ... }` initializer in `main.rs`. The
`ClaudeClient` is already constructed before `shared_deps` — pass a clone.
`health_port` is already resolved via `config.daemon.health_port` at line 796
— read it again (or extract to a local before `SharedDeps`).

### Req-3: Wire the Digest branch in orchestrator

Replace the current fall-through in the `TriggerClass::Digest` arm with an
inline async handler that calls the pipeline:

```
gather_context(...) → synthesize_digest(...) → state.should_send() check →
format_digest(...) → send to Telegram → state.record_sent(...)
```

On `synthesize_digest` error, fall back to `synthesize_digest_fallback()` and
send that result. All errors are logged; none panic.

The handler must use `&self.deps` to obtain `jira_client`, `nexus_client`,
`memory`, `calendar_credentials`, `calendar_id`, and `claude_client`.

After wiring, `TriggerClass::Digest` must no longer fall through to the worker
pool for `CronEvent::Digest`. `MorningBriefing` already returns early.

### Req-4: Add Jira result limit to gather

In `gather_jira()`, append `LIMIT 20` to the JQL string:

```
"assignee = currentUser() AND resolution = Unresolved ORDER BY priority ASC, updated DESC"
```

becomes:

```
"assignee = currentUser() AND resolution = Unresolved ORDER BY priority ASC, updated DESC LIMIT 20"
```

Cap is 20 — large enough to be useful, small enough to be prompt-safe.

### Req-5: Add max_tokens to synthesize_digest

In `synthesize_digest()`, pass `max_tokens: 1024` to `send_messages()`. The
system prompt already instructs Claude to stay under 3000 characters; the API
parameter is the hard guard. Check the `ClaudeClient::send_messages` signature
— if it does not currently accept `max_tokens`, add an optional parameter or a
new `send_messages_with_options()` variant.

### Req-6: Wire inject_budget_warning

After `synthesize_digest()` returns `Ok(result)`, check the current weekly
spend against `deps.alert_threshold_pct` and `deps.weekly_budget_usd`. If
threshold is exceeded, call `inject_budget_warning(&mut result, budget_line)`.
The budget line format: `"[Budget] ${spent:.2} / ${limit:.2} this week
({pct}%)"`. Spend is obtained from the same budget-tracking path used
elsewhere in the codebase — check `State` or the message store for the
existing spend query.

### Req-7: Fix cmd_digest() hardcoded port

Replace `let port = 8400;` with `let port = self.deps.health_port;`.

### Req-8: Fix morning briefing hour check

In `scheduler.rs`, change:

```rust
if current_hour >= MORNING_BRIEFING_HOUR
```

to:

```rust
if current_hour == MORNING_BRIEFING_HOUR
```

This ensures the briefing fires only during the 07:xx minute window, not on
any restart after 7am.

### Req-9: Batch dismiss_all_actions

Replace the N-cycle loop in `dismiss_all_actions()` with a single
load/mutate/save:

```rust
pub fn dismiss_all_actions(state_mgr: &DigestStateManager) -> Result<u32> {
    let mut state = state_mgr.load()?;
    let mut dismissed_count = 0;
    for action in state.suggested_actions.iter_mut() {
        if action.status == DigestActionStatus::Pending {
            action.status = DigestActionStatus::Dismissed;
            dismissed_count += 1;
        }
    }
    state_mgr.save(&state)?;
    Ok(dismissed_count)
}
```

`DigestStateManager::save()` must be a `pub` method — add it if it does not
exist.

## Scope
- **IN**: pipeline wiring, dead_code removal, Jira cap, max_tokens, budget
  warning wire-up, port fix, morning briefing hour fix, dismiss batch
- **OUT**: digest formatting changes, new digest sections, Telegram inline
  keyboards for digest actions, digest scheduling config changes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/digest/mod.rs` | Remove 5x `#[allow(dead_code)]` |
| `crates/nv-daemon/src/digest/gather.rs` | Add `LIMIT 20` to JQL |
| `crates/nv-daemon/src/digest/synthesize.rs` | Add `max_tokens` to Claude call |
| `crates/nv-daemon/src/digest/actions.rs` | Batch dismiss (1 load + 1 save) |
| `crates/nv-daemon/src/digest/state.rs` | Expose `save()` as `pub` if needed |
| `crates/nv-daemon/src/orchestrator.rs` | Wire Digest branch; fix cmd_digest port |
| `crates/nv-daemon/src/worker.rs` | Add `claude_client` and `health_port` to `SharedDeps` |
| `crates/nv-daemon/src/main.rs` | Populate new `SharedDeps` fields |
| `crates/nv-daemon/src/scheduler.rs` | Narrow morning briefing to `==` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Jira unavailable at digest time silently produces partial digest | Existing partial-result tolerance in `gather_context()` handles this — errors are collected and rendered in the digest |
| `synthesize_digest` fails (Claude API error) | Fall back to `synthesize_digest_fallback()`, which requires no network call |
| Budget spend query missing or expensive | If no existing spend query exists, use `0.0` as a safe default — the budget warning is informational, not blocking |
| `should_send()` suppresses first-ever digest (no prior hash) | `should_send()` returns `true` when no prior digest exists — no change needed |
| Narrowing briefing to `==` misses the window if daemon is not running at 07:xx | Existing `last_briefing_date` guard already handles restart-within-day; this fix only affects cross-day restarts after 7am |
