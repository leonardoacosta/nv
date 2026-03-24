# NV Full Audit Summary

**Date:** 2026-03-23
**Domains:** 8 | **Waves:** 3 | **Agent model:** Sonnet

## Health Scores

| Domain    | Structure | Quality | Architecture | Health | Grade |
|-----------|-----------|---------|--------------|--------|-------|
| agent     | —         | —       | —            | ~78    | C+    |
| channels  | 82        | 76      | 74           | 77     | C     |
| tools     | 78        | 82      | 75           | 78     | C+    |
| dashboard | 72        | 55      | 68           | 64     | C     |
| digest    | —         | —       | —            | N/A*   | F*    |
| watchers  | 82        | 74      | 71           | 75     | B-    |
| nexus     | 82        | 85      | 83           | 84     | B     |
| infra     | —         | —       | —            | ~76    | C+    |

*Digest is graded F because the entire pipeline is dead code — never called from runtime.

## Findings Summary

| Domain    | High | Med | Low | Total |
|-----------|------|-----|-----|-------|
| agent     | 3    | 5   | 5   | 13    |
| channels  | 3    | 4   | 4   | 11    |
| tools     | 2    | 3   | 3   | 8     |
| dashboard | 6    | 2   | 3   | 11    |
| digest    | 2    | 4   | 7   | 13    |
| watchers  | 1    | 3   | 4   | 8     |
| nexus     | 0    | 5   | 8   | 13    |
| infra     | 0    | 5   | 5   | 10    |
| **TOTAL** | **17** | **31** | **39** | **87** |

## Critical Findings (P0/P1)

### Bugs That Break User-Facing Functionality

1. **Dashboard: 5 API/frontend contract mismatches** — obligations/projects/sessions show 0,
   memory save fails (sends `path` not `topic`), settings save fails (missing `{fields:}` wrapper).
   Every dashboard page except Integrations is silently broken.

2. **Agent: parse_tool_calls() drops all but first tool call** (`claude.rs:1163`) — when Claude
   emits multiple tool calls in one turn, only the first executes. Rest silently discarded.
   Affects every multi-tool response since persistent stream path is disabled.

3. **Agent: Worker queue race condition** (`worker.rs:356`) — concurrent workers can both
   decrement active count and both spawn new workers, momentarily exceeding max_concurrent.

4. **Channels: UTF-8 panic in Telegram edit_message** (`telegram/client.rs:331`) — byte-index
   truncation on HTML text panics on non-ASCII at the 4096 boundary.

5. **Channels: Teams subscription renewal is dead code** (`teams/mod.rs:205`) — MS Graph
   subscriptions expire after 60 minutes, channel silently stops receiving after first hour.

6. **Digest: Entire pipeline is dead code** — all 5 modules carry `#[allow(dead_code)]`. The
   orchestrator's Digest branch just sends "Digest triggered" to Claude generically.

7. **Tools: 3 duplicate tool definitions** sent to Anthropic API (`mod.rs:430-456` + line 563) —
   query_nexus_health/projects/agents registered twice.

8. **Watchers: No cooldown on rule firing** — persistent external failure creates one obligation
   per watcher cycle (every 5 minutes), flooding the obligation store.

### Security Concerns

9. **Channels: Teams webhook has no clientState validation** (`http.rs:146`) — anyone who
   discovers the webhook URL can inject arbitrary Teams messages.

10. **Dashboard: GET /api/config may expose secrets** — reads config.toml and serves as JSON
    with no field filtering. Relies on secrets being in env vars only, but no enforcement.

## Domain Summaries

### Agent (13 findings)
Strong worker lifecycle and conversation management. Critical gap: cold-start tool parsing drops
multi-tool responses. ~700 lines of dead AgentLoop code. Quiet hours use wrong timezone.

### Channels (11 findings)
Clean channel isolation — each adapter runs independently. Telegram has a charset panic.
Teams subscription renewal exists but is never called. Discord gateway doesn't use Resume.
Message store has FTS5 and WAL mode. Relays need connection pooling fixes.

### Tools (8 findings)
Excellent safety patterns: Checkable trait, ServiceRegistry fallback, PendingAction confirmation
for all writes, Neon SQL injection defense. Duplicate tool definitions and env var hint mismatches
in check_services. 170K mod.rs is maintainable but large.

### Dashboard (11 findings)
Lowest health score (64). Backend API is well-structured with path traversal protection.
Frontend has zero-cost loading/error/empty states. But 5 request/response shape mismatches
mean most pages silently display wrong data. No CORS layer despite binding 0.0.0.0.

### Digest (13 findings)
**Dead code.** The gather/synthesize/format/actions/state pipeline is fully implemented and
tested but never called from the runtime. Decision needed: wire it in or delete it.
Secondary issues: UTF-8 truncation panic, unbounded Jira results, no max_tokens on Claude calls.

### Watchers (8 findings)
Clean RuleEvaluator trait. All watchers degrade gracefully on API failures. Critical gap:
no cooldown dedup — persistent failures flood obligation store. JoinHandle discarded so
watchers aren't cancelled on SIGTERM. Hardcoded /home/nyaptor in obligation detector.

### Nexus (13 findings)
Healthiest domain (84/B). Clean gRPC architecture with connection manager, streaming, watchdog.
Double failure counter accelerates quarantine (5 real attempts instead of 10). send_command
false-negative on empty output. Mutex held during 60s backoff sleep blocks concurrent operations.

### Infra (10 findings)
Solid foundation: atomic state writes, WAL mode, FTS5 search, sd_notify integration.
Health status never reflects degraded channels. Two CLI commands print "not implemented yet".
Three hardcoded /home/nyaptor paths. CPU/disk metrics slightly understated.

## Cross-Cutting Patterns

### Recurring: UTF-8 byte truncation (3 instances)
- `telegram/client.rs:331` — edit_message
- `digest/format.rs:26` — truncate_for_telegram
- `query/format.rs` — format_query_for_telegram

All use `&text[..N]` byte indexing. Fix: use `char_indices` or `floor_char_boundary`.

### Recurring: #[allow(dead_code)] masking real issues
- `agent.rs:200` — 700-line AgentLoop
- `digest/*` — entire pipeline
- `tools/mod.rs` — execute_tool() duplicate
- Multiple suppressed warnings hiding genuinely unused code

### Recurring: Hardcoded values
- Port 8400 in cmd_digest() (`orchestrator.rs:1009`)
- `/home/nyaptor` in 3+ locations
- System timezone instead of configured timezone

## Recommended Fix Order

1. Dashboard contract mismatches (6 P1 fixes, mostly frontend type unwrapping)
2. Telegram UTF-8 panic (1-line fix, prevents runtime crash)
3. Teams subscription renewal (call spawn_subscription_renewal from connect())
4. Multi-tool-call parsing in cold-start path (iterate tool_call blocks)
5. Watcher cooldown logic (read last_triggered_at before firing)
6. Decide digest pipeline fate (wire in or delete)
7. Remove dead code (AgentLoop, execute_tool, send_messages_cold_start)
8. Teams webhook clientState validation
9. Nexus double failure counter fix
10. Hardcoded values cleanup (port, paths, timezone)
