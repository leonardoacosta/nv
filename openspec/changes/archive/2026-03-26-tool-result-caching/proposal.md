# Proposal: Tool Result Caching

## Change ID
`tool-result-caching`

## Summary

Add an in-memory `ToolResultCache` with TTL-based entries that sits in front of
`execute_tool_send_with_backend` in `worker.rs`. Read-only tools return cached
results for repeated identical calls within their TTL window (default 5 minutes).
Write tools bypass the cache entirely and invalidate related read-cache entries on
success. Cache key is `(tool_name, sha256_of_input)`. Implemented as a new module
`crates/nv-daemon/src/tool_cache.rs` wired into worker dispatch.

## Context
- Extends: `crates/nv-daemon/src/worker.rs` (tool dispatch call site)
- New module: `crates/nv-daemon/src/tool_cache.rs`
- Related: `add-tool-audit-log` (audit log already wraps execute_tool_send_with_backend — cache hit/miss should be logged there too)
- Roadmap ref: Phase 3, Wave 8 — Performance / nv-4pq

## Motivation

Every tool call hits its external API regardless of whether the same query was made
seconds ago. In practice, Claude often makes 3-6 identical tool calls within a single
conversation turn — for example, querying Jira issues, Sentry errors, or Vercel
deployments while composing a digest or answering a follow-up. Each redundant call
adds 200-800ms latency and burns API rate limits.

A session-scoped in-memory cache with a 5-minute TTL for reads eliminates the
redundant round-trips without any persistence complexity. The cache lives in the
`WorkerDeps` struct and is cleared when the daemon restarts, which is the correct
scope: cached tool results are only valid for the current runtime session.

## Requirements

### Req-1: ToolResultCache Module

New file `crates/nv-daemon/src/tool_cache.rs`:

```rust
pub struct ToolResultCache {
    entries: Arc<Mutex<HashMap<CacheKey, CacheEntry>>>,
}

struct CacheKey {
    tool_name: String,
    input_hash: u64,   // FxHash or SipHash of serde_json::to_string(input)
}

struct CacheEntry {
    value: String,           // serialised ToolResult output
    expires_at: Instant,
}
```

- `ToolResultCache::new()` — constructs empty cache
- `ToolResultCache::get(tool_name, input) -> Option<String>` — returns cached value if
  entry exists and `Instant::now() < expires_at`; evicts and returns `None` if expired
- `ToolResultCache::insert(tool_name, input, value, ttl: Duration)` — stores entry with
  `expires_at = Instant::now() + ttl`
- `ToolResultCache::invalidate_prefix(prefix: &str)` — removes all entries where
  `tool_name.starts_with(prefix)`; used for write-invalidation (e.g., after
  `patch_obligation`, invalidate all `query_obligation*` entries)
- `ToolResultCache::clear()` — removes all entries (session reset)
- No background eviction thread needed — lazy eviction on `get()` is sufficient

### Req-2: TTL Policy

Define a `cache_ttl_for_tool(tool_name: &str) -> Option<Duration>` function in
`tool_cache.rs` that encodes per-tool TTL policy:

| Tool group | TTL | Rationale |
|---|---|---|
| `query_obligations`, `list_obligations` | 5 min | Read-only obligation store |
| `jira_*` (except `jira_create`, `jira_update`, `jira_transition`) | 5 min | External API, slow |
| `gh_pr_list`, `gh_run_status`, `gh_issues`, `gh_releases` | 3 min | CI state changes frequently |
| `gh_pr_detail`, `gh_pr_diff`, `gh_compare` | 10 min | PR content is stable |
| `sentry_issues`, `sentry_issue` | 5 min | Error lists are stable |
| `vercel_deployments`, `vercel_logs` | 3 min | Deploy state changes frequently |
| `neon_projects`, `neon_branches`, `neon_compute` | 10 min | Infra metadata is stable |
| `docker_status`, `docker_logs` | 1 min | Container state can change quickly |
| `doppler_secrets`, `doppler_compare`, `doppler_activity` | 10 min | Secrets rarely change |
| `cloudflare_*` | 10 min | DNS rarely changes |
| `posthog_flags` | 5 min | Feature flags are stable within session |
| `calendar_today`, `calendar_upcoming`, `calendar_next` | 5 min | Calendar is read-only |
| `ha_states`, `ha_entity` | 1 min | Home state changes frequently |
| `plaid_balances`, `plaid_bills` | 5 min | Financial data read-only |
| `stripe_customers`, `stripe_invoices` | 5 min | Read-only Stripe queries |
| `upstash_info`, `upstash_keys` | 2 min | Cache/queue state changes |
| `resend_emails`, `resend_bounces` | 5 min | Email log read-only |
| `neon_query` | 0 (no cache) | SQL queries may have side effects |
| `fetch_url`, `check_url`, `search_web` | 0 (no cache) | Web content is not stable |
| All write/mutation tools | 0 (no cache) | Never cache writes |
| All other tools not listed | 0 (no cache) | Conservative default: opt-in only |

Returns `None` (zero TTL / no cache) for any tool not in the table above.

### Req-3: Write Invalidation Map

Define `invalidation_prefix_for_tool(tool_name: &str) -> Option<&'static str>` in
`tool_cache.rs` mapping write tools to the read-cache prefix they invalidate:

| Write tool | Invalidates prefix |
|---|---|
| `patch_obligation`, `create_obligation`, `close_obligation` | `"query_obligation"` / `"list_obligation"` |
| `jira_create_issue`, `jira_update_issue`, `jira_transition_issue` | `"jira_"` |
| `ha_service_call` | `"ha_"` |

Other write tools do not have corresponding read tools that would be cached, so no
invalidation is needed for them.

### Req-4: Worker Integration

In `worker.rs`, inside the tool dispatch block (around line 1803, before the
`tokio::time::timeout(... execute_tool_send_with_backend(...))` call):

1. Call `deps.tool_cache.get(name, input)` — if `Some(cached)`, skip the external
   call and use the cached string as the result directly (wrap as
   `ToolResult::Immediate(cached)`). Increment a `cache_hits` counter in audit log.
2. On cache miss: execute normally, then on `Ok(ToolResult::Immediate(output))`,
   call `deps.tool_cache.insert(name, input, output.clone(), ttl)` if
   `cache_ttl_for_tool(name).is_some()`.
3. On `Ok(_)` for any write tool that matches `invalidation_prefix_for_tool(name)`,
   call `deps.tool_cache.invalidate_prefix(prefix)` after the successful call.
4. Cache operations must not be gated on the timeout — if the timeout fires, no
   cache write occurs.

`ToolResultCache` is `Clone` (wraps `Arc<Mutex<...>>`), so it can be added to
`WorkerDeps` without lifetime changes.

### Req-5: WorkerDeps Integration

Add `tool_cache: ToolResultCache` field to `WorkerDeps` (wherever the deps struct
is defined — likely `state.rs` or `worker.rs`). Initialise with
`ToolResultCache::new()` in the daemon startup path.

### Req-6: Audit Log Extension

Extend the existing tool audit log call (in worker.rs around line 1839) to record a
`cache_hit: bool` field alongside the existing fields. When the result came from
cache, `duration_ms` should reflect near-zero (the cache lookup time), not the
original API call duration.

### Req-7: No Persistence

The cache is entirely in-memory. No SQLite, no disk writes, no cross-session sharing.
Daemon restart clears all entries. This is intentional — stale cached data across
sessions would be harder to debug than a cold start.

## Scope
- **IN**: `tool_cache.rs` module, TTL policy table, write invalidation, worker
  integration, `WorkerDeps` field, audit log `cache_hit` column
- **OUT**: Distributed cache (Redis/Upstash), persistent cross-session cache,
  per-user TTL configuration, cache size limits/LRU eviction, metrics dashboard,
  cache warming on startup

## Impact
| Area | Change |
|---|---|
| `crates/nv-daemon/src/tool_cache.rs` | New: ToolResultCache struct, get/insert/invalidate_prefix/clear, cache_ttl_for_tool(), invalidation_prefix_for_tool() |
| `crates/nv-daemon/src/worker.rs` | Add cache lookup before dispatch, cache write after success, invalidation after writes |
| `crates/nv-daemon/src/state.rs` | Add `tool_cache: ToolResultCache` field to WorkerDeps (or wherever WorkerDeps is defined) |
| `crates/nv-daemon/src/lib.rs` or `main.rs` | Add `mod tool_cache;` declaration |

## Risks
| Risk | Mitigation |
|---|---|
| Stale data served during TTL window | TTL values are conservative (1-10 min). User can restart daemon to clear. Cache is opt-in per tool. |
| Cache incorrectly applied to writes | `cache_ttl_for_tool` returns `None` for all write/mutation tools. Write tools only appear in invalidation map, never in cache insert path. |
| `Mutex` contention on hot path | Cache lookup is a single HashMap get (~100ns). Lock is held only for the lookup, not during the API call. No contention risk at Nova's usage scale. |
| Input hash collisions | Using std `DefaultHasher` or `SipHash`; probability negligible for tool inputs. Worst case: stale result served for one TTL window. |
| `ToolResult::PendingAction` cached incorrectly | Only cache `ToolResult::Immediate` variants. `PendingAction` results are never written to cache. |
