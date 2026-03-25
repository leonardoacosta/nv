# Implementation Tasks

<!-- beads:epic:nv-8rem -->

## Rust Implementation

- [ ] [1.1] [P-1] Create `crates/nv-daemon/src/tool_cache.rs` — define `CacheKey` (tool_name: String, input_hash: u64), `CacheEntry` (value: String, expires_at: Instant), and `ToolResultCache` wrapping `Arc<Mutex<HashMap<CacheKey, CacheEntry>>>` [owner:api-engineer]
- [ ] [1.2] [P-1] Implement `ToolResultCache::new()`, `get(tool_name, input) -> Option<String>` (lazy eviction on expired entries), `insert(tool_name, input, value, ttl: Duration)`, `invalidate_prefix(prefix: &str)`, `clear()` [owner:api-engineer]
- [ ] [1.3] [P-1] Implement `cache_ttl_for_tool(tool_name: &str) -> Option<Duration>` — full TTL policy table from Req-2 (obligation/jira/gh/sentry/vercel/neon/docker/doppler/cf/posthog/calendar/ha/plaid/stripe/upstash/resend; returns None for neon_query, web tools, writes, and all unlisted tools) [owner:api-engineer]
- [ ] [1.4] [P-2] Implement `invalidation_prefix_for_tool(tool_name: &str) -> Option<&'static str>` — mapping for patch_obligation/create_obligation/close_obligation -> "query_obligation"/"list_obligation", jira write tools -> "jira_", ha_service_call -> "ha_" [owner:api-engineer]
- [ ] [1.5] [P-1] Add `pub mod tool_cache;` declaration in `crates/nv-daemon/src/lib.rs` or `main.rs` (whichever owns module declarations) [owner:api-engineer]

## WorkerDeps Integration

- [ ] [2.1] [P-1] Add `tool_cache: ToolResultCache` field to `WorkerDeps` struct (locate in state.rs or worker.rs) — derive `Clone` on `ToolResultCache` (it wraps Arc, so Clone is cheap) [owner:api-engineer]
- [ ] [2.2] [P-1] Initialise `tool_cache: ToolResultCache::new()` in the daemon startup path where WorkerDeps is constructed [owner:api-engineer]

## Worker Dispatch Integration

- [ ] [3.1] [P-1] In `worker.rs` tool dispatch block: before `tokio::time::timeout(... execute_tool_send_with_backend(...))`, call `deps.tool_cache.get(name, input)` — on `Some(cached)`, short-circuit with `Ok(ToolResult::Immediate(cached))` and set `cache_hit = true` [owner:api-engineer]
- [ ] [3.2] [P-1] After successful `Ok(ToolResult::Immediate(output))` from execute_tool_send_with_backend: if `cache_ttl_for_tool(name).is_some()`, call `deps.tool_cache.insert(name, input, output.clone(), ttl)` [owner:api-engineer]
- [ ] [3.3] [P-2] After successful result from a write tool: if `invalidation_prefix_for_tool(name).is_some()`, call `deps.tool_cache.invalidate_prefix(prefix)` [owner:api-engineer]
- [ ] [3.4] [P-2] Extend existing audit log call (around line 1839 in worker.rs) — add `cache_hit: bool` to the log_tool_usage() call; when result is from cache, `duration_ms` reflects cache lookup time only [owner:api-engineer]

## Verify

- [ ] [4.1] cargo build passes [owner:api-engineer]
- [ ] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [4.3] Unit test: `get()` returns None on empty cache [owner:api-engineer]
- [ ] [4.4] Unit test: `insert()` then `get()` returns cached value within TTL [owner:api-engineer]
- [ ] [4.5] Unit test: `get()` returns None after TTL expires (use short TTL + sleep or mock Instant) [owner:api-engineer]
- [ ] [4.6] Unit test: `invalidate_prefix("jira_")` removes all jira_* entries, leaves unrelated entries intact [owner:api-engineer]
- [ ] [4.7] Unit test: `cache_ttl_for_tool("jira_search")` returns Some(5min); `cache_ttl_for_tool("jira_create_issue")` returns None; `cache_ttl_for_tool("neon_query")` returns None; `cache_ttl_for_tool("fetch_url")` returns None [owner:api-engineer]
- [ ] [4.8] Unit test: `ToolResult::PendingAction` result is NOT written to cache (only Immediate is cached) [owner:api-engineer]
- [ ] [4.9] Existing tests pass [owner:api-engineer]
- [ ] [4.10] [user] Manual test: ask Nova the same read-only question twice within 5 minutes (e.g. "list Jira issues"), verify second response arrives with near-zero latency and audit log shows cache_hit=true [owner:api-engineer]
