---
name: audit:agent
description: Audit the core agent loop — orchestrator, worker pool, Claude API, conversation management
type: command
execution: foreground
---

# Audit: Agent Core

Audit the AI agent loop: trigger classification, worker dispatch, Claude API interaction,
conversation management, and response routing.

## Scope

| Module | File | Purpose |
|--------|------|---------|
| Orchestrator | `crates/nv-daemon/src/orchestrator.rs` | Trigger classification & dispatch |
| Worker Pool | `crates/nv-daemon/src/worker.rs` | Concurrent task execution with priority queue |
| Claude Client | `crates/nv-daemon/src/claude.rs` | Anthropic API/CLI client with retry logic |
| Conversation | `crates/nv-daemon/src/conversation.rs` | History management (20 turns, 50K chars, 10min timeout) |
| Agent Bootstrap | `crates/nv-daemon/src/agent.rs` | System prompt, context building, bootstrap state |
| HTTP Entry | `crates/nv-daemon/src/http.rs` | `POST /ask` endpoint |

## Routes

| # | Method | Path | What to check |
|---|--------|------|---------------|
| 1 | POST | `/ask` | Request/response flow, error handling, timeout behavior |

## Audit Checklist

### Orchestrator
- [ ] `classify_trigger()` covers all `TriggerClass` variants (Command, Query, Chat, Digest, Callback, NexusEvent, BotCommand)
- [ ] Bot command parsing handles all registered commands
- [ ] Quiet hours logic correct (timezone-aware)
- [ ] Telegram formatting doesn't break on edge cases (long text, special chars)

### Worker Pool
- [ ] Priority queue ordering (High before Normal)
- [ ] Concurrency limits enforced
- [ ] Tool timeout enforcement (30s read, 60s write)
- [ ] Worker event lifecycle (StageStarted → ToolCalled → StageComplete → Complete/Error)
- [ ] Graceful shutdown behavior

### Claude Client
- [ ] Rate limit backoff/retry logic
- [ ] Streaming vs non-streaming request paths
- [ ] Token counting and budget enforcement
- [ ] Error handling for API failures (network, auth, rate limit)

### Conversation
- [ ] History truncation at MAX_HISTORY_TURNS (20) and MAX_HISTORY_CHARS (50K)
- [ ] Tool result truncation to 1KB
- [ ] Session timeout (600s) auto-clear
- [ ] Concurrent access safety

### Agent Bootstrap
- [ ] System prompt loading from `~/.nv/system-prompt.md` with fallback
- [ ] Memory integration in context building
- [ ] Bootstrap interview flow for first-run

## Memory

Persist findings to: `.claude/audit/memory/agent-memory.md`

## Findings

Log structured findings to: `~/.claude/scripts/state/nv-audit-findings.jsonl`

Format:
```json
{"domain":"agent","severity":"high|medium|low","category":"bug|risk|improvement","finding":"description","file":"path","line":0}
```
