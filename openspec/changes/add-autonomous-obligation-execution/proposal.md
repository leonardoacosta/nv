# Proposal: Add Autonomous Obligation Execution

## Change ID
`add-autonomous-obligation-execution`

## Summary

Enable Nova to autonomously work on her own obligations when idle — picking up the highest-priority
open obligation, executing it using all available tools (up to 30 per attempt), reporting results
via Telegram, and proposing completion for Leo's confirmation.

## Context
- Extends: `crates/nv-daemon/src/orchestrator.rs` (idle detection, execution dispatch),
  `crates/nv-daemon/src/worker.rs` (tool execution), `crates/nv-daemon/src/obligation_store.rs`
  (status transitions)
- Related: `proactive-followups` (v8 — watcher scans obligations), `proactive-obligation-research`
  (v8 — gathers context), `migrate-nova-brain` (v7 — dashboard forwarding)
- Depends on: obligation_store, worker pool, tool dispatch system (all exist)

## Motivation

Nova currently detects obligations, stores them, reminds Leo about stale ones, and gathers
background research — but never actually *works* on them. An obligation like "Pull two weeks of
Teams messages and build org intelligence profiles" sits in `status: open` with `owner: nova`
indefinitely, waiting for Leo to manually trigger the work.

This is the gap between a task tracker and an autonomous agent. Nova has all the tools she needs
(Teams API, Jira, GitHub, calendar, etc.) — she just never picks them up and uses them on her own
behalf.

## Requirements

### Req-1: Idle Detection

Add idle detection to the orchestrator. Nova is "idle" when:
- No interactive messages are currently being processed by the worker pool
- No worker tasks are in-flight (active worker count = 0)
- The last interactive message completed at least 60 seconds ago (debounce to avoid
  interrupting conversation flow)

Idle detection runs on a 30-second poll cycle inside the orchestrator's main loop. When idle is
detected and there are open Nova obligations, trigger obligation execution.

### Req-2: Obligation Picker

When idle, select the highest-priority open obligation owned by Nova:
- Filter: `owner = "nova"` AND `status IN ("open", "in_progress")`
- Sort: priority ASC (P1 first), then created_at ASC (oldest first)
- Skip: obligations with `last_attempt_at` within the last 2 hours (cooldown to prevent
  retry loops on stuck obligations)
- Pick: first matching obligation

Add `last_attempt_at: Option<DateTime>` column to obligations table (migration).

### Req-3: Obligation Executor

Create `crates/nv-daemon/src/obligation_executor.rs` — the core execution engine.

`execute_obligation(obligation: &Obligation, deps: &SharedDeps) -> ObligationResult`

The executor:
1. Builds a system context including: the obligation's `detected_action`, `source_message`,
   any existing `obligation_notes` (from research spec), the obligation's priority and project
2. Sends a single Claude turn with system prompt: "You are Nova. You have an obligation to
   complete: {detected_action}. Use your tools to fulfill this obligation. When done, summarize
   what you accomplished."
3. Runs the tool loop with NO tool count cap (same pattern as `Worker::run`), bounded only by
   the 5-minute timeout
4. Captures the final response text as the execution result
5. Updates `last_attempt_at` on the obligation

The executor uses the SAME tool dispatch as interactive messages — full tool access, no gates,
no PendingAction barriers. Nova has full autonomy.

### Req-4: Result Reporting

After execution completes (success or failure):

**On success (non-empty response):**
1. Store result in obligation_notes: `"[Auto-executed {timestamp}] {response}"`
2. Send Telegram message to Leo: brief summary of what was done (first 500 chars of response)
3. Set obligation status to `proposed_done`

**On failure (error, timeout, empty response):**
1. Store error in obligation_notes: `"[Attempt failed {timestamp}] {error}"`
2. Send Telegram message: "Failed to complete: {detected_action} — {error summary}"
3. Keep status as `in_progress` (don't revert to open)
4. The 2-hour cooldown (Req-2) prevents immediate retry

### Req-5: Proposed Done Status

Add `proposed_done` as a new obligation status value.

When an obligation is in `proposed_done`:
- The proactive watcher skips it (no reminders)
- The dashboard shows it with a "Verify" badge
- A Telegram inline keyboard is sent with: `[Confirm Done] [Reopen]`
- `Confirm Done` callback → transitions to `done`
- `Reopen` callback → transitions to `open` (Nova will re-attempt on next idle cycle)

### Req-6: Safety Guards

- **Timeout**: Max 5 minutes per obligation attempt. If exceeded, kill the worker task and report
  timeout. No tool count cap — Nova uses as many tools as needed within the time budget.
- **Cooldown**: 2 hours between attempts on the same obligation. Prevents infinite retry loops.
- **One at a time**: Only one obligation executes at a time during idle. When it completes, check
  idle again before picking the next one.
- **Interactive preemption**: If an interactive message arrives during obligation execution, the
  obligation worker continues (it's already in-flight) but no new obligation work starts until
  idle resumes.

### Req-7: Configuration

Add to `nv.toml`:
```toml
[autonomy]
enabled = true
timeout_secs = 300
cooldown_hours = 2
idle_debounce_secs = 60
```

All fields optional with defaults shown.

## Scope
- **IN**: Idle detection, obligation picker, executor with full tool access, result reporting via
  Telegram, proposed_done status + confirmation callbacks, safety guards, config
- **OUT**: Multi-obligation parallel execution, obligation decomposition (breaking one obligation
  into sub-tasks), learning from past attempts, priority auto-adjustment

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/obligation_executor.rs` | New: core execution engine |
| `crates/nv-daemon/src/orchestrator.rs` | Idle detection loop, obligation dispatch |
| `crates/nv-daemon/src/obligation_store.rs` | `proposed_done` status, `last_attempt_at` column, migration |
| `crates/nv-daemon/src/worker.rs` | Expose tool dispatch for executor reuse |
| `crates/nv-daemon/src/callbacks.rs` | `confirm_done:` and `reopen:` callback handlers |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | Callback label for confirm/reopen |
| `crates/nv-core/src/config.rs` | `AutonomyConfig` struct |
| `config/nv.toml` | `[autonomy]` section |

## Risks

| Risk | Mitigation |
|------|-----------|
| Nova enters infinite tool loop on a vague obligation | 30-tool budget + 5-min timeout hard caps |
| Nova takes destructive action (deletes data, sends wrong message) | Full autonomy is user's explicit choice; review via proposed_done before closing |
| Obligation execution costs too many tokens | Tool budget bounds per-attempt cost; cooldown prevents rapid retries |
| Idle detection too aggressive (starts work during conversation pause) | 60-second debounce after last interactive message |
| Stuck obligation blocks all autonomous work | 2-hour cooldown ensures other obligations get a turn |
