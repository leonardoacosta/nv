# Agent Domain Audit — Memory

**Audited:** 2026-03-23  
**Scope:** orchestrator.rs, worker.rs, claude.rs, conversation.rs, agent.rs, http.rs

---

## Checklist Results

### Orchestrator (orchestrator.rs)

| Check | Result | Notes |
|-------|--------|-------|
| classify_trigger() covers all TriggerClass variants | PASS | Cron→Digest, NexusEvent, CliCommand→Command, Message→(Callback/BotCommand/Chat/Command/Query) |
| Bot command parsing handles all registered commands | PASS | /status, /digest, /health, /projects, /apply; unknown → help text |
| Quiet hours logic correct (timezone-aware) | CONCERN | Uses `chrono::Local::now().time()` (system TZ) but `SharedDeps.timezone` is an IANA string. If server TZ differs from user TZ, quiet hours fire at wrong time. |
| Telegram formatting edge cases | PASS | format_for_telegram() strips unsupported markdown; empty response deletes thinking indicator |

### Worker Pool (worker.rs)

| Check | Result | Notes |
|-------|--------|-------|
| Priority queue ordering (High before Normal) | PASS | BinaryHeap with custom Ord: higher priority value first, then older tasks first (FIFO within tier). Tests cover this. |
| Concurrency limits enforced | CONCERN | Race window: active is decremented before re-checking whether next queued task can be dequeued. Under max_concurrent > 1 with rapid completions, the active counter and queue pop are not atomic. |
| Tool timeout enforcement (30s read, 60s write) | PASS | WRITE_TOOLS constant list; tokio::time::timeout wraps execute_tool_send |
| Worker event lifecycle (StageStarted→ToolCalled→StageComplete→Complete/Error) | PASS | All 5 variants emitted; orchestrator cleans up on Complete and Error |
| Graceful shutdown | PASS | Worker pool doesn't hold a shutdown handle — workers drain naturally. Orchestrator exits when trigger_rx closed AND worker_stage_started is empty. |

### Claude Client (claude.rs)

| Check | Result | Notes |
|-------|--------|-------|
| Rate limit backoff/retry logic | CONCERN | Only 1 cold-start retry on JSON parse failure (1s sleep). No rate-limit detection or exponential backoff at the client level. Rate limit messages are surfaced as user-facing errors via the worker, which is fine, but no automatic retry-after. |
| Streaming vs non-streaming paths | PASS | PersistentSession (stream-json) with fallback_only=true currently forces cold-start. Stream path exists but disabled pending CC 2.1.81 fix. |
| Token counting and budget enforcement | PASS | Token counts returned from CLI usage field; budget threshold alert at configurable pct via worker.rs |
| Error handling for API failures | PASS | Auth errors, CLI errors, IO errors all classified. User-facing messages differentiate rate limits, timeouts, auth, generic. |

**Bug found:** parse_tool_calls() only parses the FIRST ```tool_call block. Multiple tool calls in one response silently drop all but the first.

### Conversation (conversation.rs)

| Check | Result | Notes |
|-------|--------|-------|
| History truncation at MAX_HISTORY_TURNS (20) | PASS | trim() enforces both limits; tests verify |
| History truncation at MAX_HISTORY_CHARS (50K) | PASS | total_chars() sums all turns; removes oldest first |
| Tool result truncation to 1KB | PASS | truncate_tool_results() called on push; handles UTF-8 boundaries correctly |
| Session timeout (600s) auto-clear | PASS | load() checks elapsed >= SESSION_TIMEOUT; clears and returns empty |
| Concurrent access safety | PASS | Wrapped in std::sync::Mutex<ConversationStore> at usage sites; no lock held across await |

### Agent Bootstrap (agent.rs)

| Check | Result | Notes |
|-------|--------|-------|
| System prompt loading from ~/.nv/system-prompt.md with fallback | PASS | load_system_prompt() tries override path, falls back to DEFAULT_SYSTEM_PROMPT const |
| Memory integration in context building | PASS | build_system_context() appends identity.md, soul.md, user.md and injects memory file listing |
| Bootstrap interview flow for first-run | PASS | check_bootstrap_state() gates tool registration; bootstrap.md loaded instead of identity/soul |

**Dead code found:** Entire AgentLoop struct + impl (~700 lines, all #[allow(dead_code)]) is never used. The real execution path is Worker/WorkerPool/Orchestrator.

---

## Findings Summary

### High Severity (3)

1. **parse_tool_calls() parses only the first tool_call block** — silent data loss when Claude emits multiple tools in one cold-start turn. `claude.rs:1163`

2. **Worker queue dequeue race condition** — active counter and queue pop are not atomic; max_concurrent can be exceeded under load. `worker.rs:356`

3. **Queued-worker timeout missing error event + user notification** — the primary worker's timeout handler emits a WorkerEvent::Error and sends a Telegram message; the secondary queued-worker spawn only logs. `worker.rs:386`

### Medium Severity (5)

4. **Quiet hours use system timezone, not user timezone** — `is_quiet_hours()` calls `chrono::Local::now()` while `deps.timezone` is IANA. `orchestrator.rs:1775`

5. **cmd_digest() hardcodes port 8400** — breaks if daemon port is configured differently. `orchestrator.rs:1009`

6. **StageComplete clears stage tracking before ToolCalled events** — brief gap where active workers appear idle to check_inactivity(). `orchestrator.rs:660`

7. **AgentLoop is ~700 lines of dead code** — entire struct + impl never instantiated; duplicates Worker's context-build, callback, and tool-loop logic. `agent.rs:200`

8. **truncate_history() duplicated with differing implementations** — agent.rs:1304 and worker.rs:1632 differ; risk of divergence. `worker.rs:1632`

### Low Severity (5)

9. **extract_cli_response_channels() duplicated** in agent.rs and orchestrator.rs.

10. **send_messages_cold_start() dead code** — superseded by with_image variant. `claude.rs:755`

11. **flush_error_batch_if_expired() never called** — last error in a sequence is silently dropped. `orchestrator.rs:1427`

12. **Tool results in live conversation_history not truncated** — only ConversationStore.push() truncates; active-turn history can contain arbitrarily large results. `worker.rs:1262`

13. **POST /ask timeout (60s) shorter than worker_timeout_secs (300s default)** — CLI callers get 504 while agent is still processing. `http.rs:225`

---

## Architecture Notes

- Worker pool uses `BinaryHeap<PrioritizedTask>` behind `std::sync::Mutex` — correct for priority ordering, but the mutex is held briefly for push/pop only, not across processing.
- ConversationStore is single-user (one conversation across all workers), meaning concurrent workers share one conversation history. This is intentional — Nova has one "session" — but means interleaved turns from multiple concurrent workers will corrupt each other's conversation context.
- The persistent session (stream-json) is force-disabled via `fallback_only: true` at construction time with a code comment citing a CC 2.1.81 bug. This is a standing TODO once the upstream bug is fixed.
