# Proposal: Add Upstash Tools

## Change ID
`add-upstash-tools`

## Summary

Upstash Redis info tools via REST API. Two read-only tools (`upstash_info`, `upstash_keys`)
that query Upstash Redis for server info and key listings, enabling Nova to monitor Redis
health and inspect cached data.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution)
- Related: Existing tool pattern (Jira, Nexus, Memory tools), `add-tool-audit-log` spec (audit logging)
- PRD ref: Phase 2, Section 6.1 — Tier 3 (URL + token)

## Motivation

Upstash Redis is used for rate limiting, caching, and queuing across projects. Currently
there's no way to check Redis health, key count, or memory usage without using the Upstash
console or running redis-cli. Wiring Upstash into Nova lets Leo ask "How's Redis doing?" or
"What keys match session:*?" from Telegram.

## Requirements

### Req-1: HTTP Client Module

New file `crates/nv-daemon/src/upstash.rs` with:
- `UpstashClient` struct holding REST URL and token
- Auth: Upstash REST API uses `Authorization: Bearer $UPSTASH_REDIS_REST_TOKEN` header
- Base URL: `$UPSTASH_REDIS_REST_URL` (e.g., `https://xyz.upstash.io`)
- Commands sent as POST to base URL with JSON body `["COMMAND", "arg1", "arg2"]`

### Req-2: upstash_info Tool

`upstash_info()` — Returns Redis server info.

- Command: `INFO` via REST API
- Output: Formatted summary — connected clients, used memory, keyspace hits/misses, uptime
- Parse the INFO response string into key sections

### Req-3: upstash_keys Tool

`upstash_keys(pattern)` — List keys matching a glob pattern.

- Command: `KEYS <pattern>` via REST API (or `SCAN 0 MATCH <pattern> COUNT 100` for safety)
- Input: `pattern` (required) — glob pattern (e.g., `"session:*"`, `"ratelimit:*"`)
- Output: List of matching key names, capped at 100
- Warning: KEYS can be slow on large datasets. Use SCAN if Upstash supports it.

### Req-4: Tool Registration

Register both tools in `register_tools()` with Anthropic tool schema format.
Wire dispatch in `execute_tool()` to call UpstashClient methods.

### Req-5: Configuration

- Env vars: `UPSTASH_REDIS_REST_URL` + `UPSTASH_REDIS_REST_TOKEN`
- Add to config or read from env directly
- Fail gracefully: if either var is missing, tools return "Upstash not configured"

### Req-6: Audit Logging

Every tool invocation logged via tool audit log. Log: tool name, pattern (for keys), success/failure, duration_ms.

## Scope
- **IN**: UpstashClient HTTP module, upstash_info tool, upstash_keys tool, tool registration, env config
- **OUT**: Writing/deleting keys, pub/sub, Upstash Kafka/QStash, database management API

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/upstash.rs` | New: UpstashClient with info(), keys(pattern) |
| `crates/nv-daemon/src/tools.rs` | Add 2 tool definitions + dispatch cases |
| `crates/nv-daemon/src/main.rs` | Init UpstashClient, pass to tool executor |
| `config/env` or `.env` | Add UPSTASH_REDIS_REST_URL, UPSTASH_REDIS_REST_TOKEN |

## Risks
| Risk | Mitigation |
|------|-----------|
| KEYS command on large dataset | Prefer SCAN with COUNT 100. Cap results at 100 keys. |
| REST API latency | Upstash REST is ~5ms. Set 10s timeout as safety net. |
| Token leaked in logs | Never log token. Log only command name + pattern. |
