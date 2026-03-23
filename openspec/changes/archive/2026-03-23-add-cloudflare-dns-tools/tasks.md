# Implementation Tasks

<!-- beads:epic:TBD -->

## Module Setup

- [x] [1.1] [P-1] Create `crates/nv-daemon/src/cloudflare_tools.rs` — CloudflareClient struct with `from_env()` (reads `CLOUDFLARE_API_TOKEN`), reqwest client, 15s timeout, base URL constant, Cloudflare API response types (Zone, DnsRecord, SettingValue, ApiResponse<T> envelope) [owner:api-engineer]
- [x] [1.2] [P-1] Add `mod cloudflare_tools;` to `main.rs` [owner:api-engineer]
- [x] [1.3] [P-1] Add `use crate::cloudflare_tools;` to `tools.rs`, call `tools.extend(cloudflare_tools::cloudflare_tool_definitions())` in `register_tools()` [owner:api-engineer]

## Zone Cache

- [x] [2.1] [P-1] Implement zone cache — `static Mutex<Option<(Instant, Vec<Zone>)>>` with 1-hour TTL. Internal `fetch_zones()` populates cache from `GET /zones?per_page=50` with pagination. `get_cached_zones()` returns cached data or fetches fresh. `resolve_zone_id(domain)` looks up domain in cache, returns zone_id or error listing available zones [owner:api-engineer]

## Tool Definitions

- [x] [3.1] [P-1] Add `cloudflare_tool_definitions() -> Vec<ToolDefinition>` — three tool schemas: `cf_zones` (no required params), `cf_dns_records` (required: domain, optional: record_type), `cf_domain_status` (required: domain) [owner:api-engineer]

## Tool Handlers

- [x] [4.1] [P-1] Implement `cf_zones()` — fetch zones via cache, format as text table (Zone, Status, Plan, Nameservers columns) [owner:api-engineer]
- [x] [4.2] [P-1] Implement `cf_dns_records(domain, record_type)` — resolve domain to zone_id, `GET /zones/{zone_id}/dns_records` with optional type filter, format as text table (Type, Name, Content, Proxied, TTL), show `auto` for TTL=1 [owner:api-engineer]
- [x] [4.3] [P-1] Implement `cf_domain_status(domain)` — resolve domain, fetch SSL setting and security_level setting (parallel with `tokio::join!`), format as structured text. Show `unknown` for individual field failures [owner:api-engineer]

## Tool Dispatch

- [x] [5.1] [P-1] Add `cf_zones`, `cf_dns_records`, `cf_domain_status` dispatch arms to `execute_tool()` in `tools.rs` — create client via `from_env()`, call handlers, return `ToolResult::Immediate` [owner:api-engineer]
- [x] [5.2] [P-1] Add matching dispatch arms to `execute_tool_send()` in `tools.rs` [owner:api-engineer]

## Orchestrator Integration

- [x] [6.1] [P-2] Add `"cf_zones" | "cf_dns_records" | "cf_domain_status" => "Checking Cloudflare DNS..."` to `humanize_tool()` in `orchestrator.rs` [owner:api-engineer]

## Verify

- [x] [7.1] `cargo build` passes [owner:api-engineer]
- [x] [7.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [7.3] `cargo test` — existing tests pass [owner:api-engineer]
