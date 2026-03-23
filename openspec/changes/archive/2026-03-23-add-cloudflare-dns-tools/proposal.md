# Proposal: Cloudflare DNS Tools

## Change ID
`add-cloudflare-dns-tools`

## Summary

Add three read-only Cloudflare DNS tools to Nova: `cf_zones` (list all zones with status, cached
1 hour), `cf_dns_records` (list DNS records for a domain, with type filter), and
`cf_domain_status` (quick health check — nameservers, SSL mode, proxy status, security level).
All tools authenticate via `CLOUDFLARE_API_TOKEN` env var against the Cloudflare API v4.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (register_tools, execute_tool, execute_tool_send), `crates/nv-daemon/src/main.rs` (mod declaration), `crates/nv-daemon/src/orchestrator.rs` (humanize_tool)
- Pattern: follows existing tool modules (`vercel_tools.rs`, `neon_tools.rs`, `sentry_tools.rs`) — each module exposes `*_tool_definitions()` + public async handler functions, dispatched from `tools.rs`
- Related: read-only API tool pattern established by Vercel, Sentry, Stripe, Neon modules — Bearer token auth via env var, `reqwest` client, typed deserialization, formatted text output
- Depends on: nothing — standalone addition

## Motivation

Nova currently has no visibility into DNS infrastructure. When troubleshooting domain issues,
checking nameserver propagation, or auditing SSL configuration, the operator must leave the
conversation to open the Cloudflare dashboard. Adding read-only DNS tools lets Nova answer
questions like "what DNS records exist for example.com?", "is the domain proxied?", and
"what's the SSL mode?" directly in chat. This completes Nova's infrastructure observability
alongside the existing Vercel, Sentry, and Docker tools.

## Requirements

### Req-1: `CLOUDFLARE_API_TOKEN` Authentication

All Cloudflare tools share a single HTTP client authenticated via the `CLOUDFLARE_API_TOKEN`
environment variable. The token needs `Zone:Read` and `DNS:Read` permissions.

- Read the token from `std::env::var("CLOUDFLARE_API_TOKEN")` at call time (same pattern as
  `VercelClient::from_env()`)
- Use `reqwest::Client` with `Authorization: Bearer <token>` header
- Base URL: `https://api.cloudflare.com/client/v4`
- Request timeout: 15 seconds
- Return a clear error if the env var is not set: `"Cloudflare not configured — CLOUDFLARE_API_TOKEN not set"`

### Req-2: `cf_zones` — List All Zones

List all Cloudflare zones (domains) with their status. Cache the zone list for 1 hour to avoid
redundant API calls (zones rarely change).

- API: `GET /zones?per_page=50&page=1` (paginate if >50 zones)
- Cache: `std::sync::Mutex<Option<(Instant, Vec<Zone>)>>` — static or lazy_static. Return cached
  data if less than 1 hour old. No cache invalidation parameter needed.
- Response fields per zone: `name` (domain), `status` (active/pending/moved/deactivated),
  `plan.name` (Free/Pro/Business/Enterprise), `name_servers[]`
- Format output as a text table:

```
Zone                  Status    Plan        Nameservers
example.com           active    Pro         aria.ns.cloudflare.com, bob.ns.cloudflare.com
other.dev             active    Free        aria.ns.cloudflare.com, bob.ns.cloudflare.com
```

- Input schema: no required parameters (list all zones)
- The zone cache is also used internally by `cf_dns_records` and `cf_domain_status` to resolve
  domain names to zone IDs without extra API calls

### Req-3: `cf_dns_records` — List DNS Records for a Zone

List DNS records for a given domain. Accept a domain name (not a zone ID) — resolve to zone_id
internally using the zone cache from Req-2.

- Input: `domain` (required, string), `record_type` (optional, string — e.g. "A", "CNAME", "MX")
- Zone resolution: look up `domain` in the cached zone list (Req-2). If not cached or expired,
  fetch zones first. If the domain is not found, return a clear error listing available zones.
- API: `GET /zones/{zone_id}/dns_records?per_page=100` with optional `&type={record_type}` filter
- Response fields per record: `type`, `name`, `content`, `proxied` (boolean), `ttl`
- Format output as a text table:

```
Type    Name                Content                  Proxied  TTL
A       example.com         203.0.113.1              yes      auto
CNAME   www.example.com     example.com              yes      auto
MX      example.com         mail.example.com         no       3600
TXT     example.com         v=spf1 include:...       no       auto
```

- TTL display: show `auto` when TTL is 1 (Cloudflare's auto value), otherwise show seconds
- Proxied display: `yes` / `no`

### Req-4: `cf_domain_status` — Quick Domain Health Check

Return a concise health summary for a domain: nameserver status, SSL mode, proxy status, and
security level. Designed for quick "is everything OK?" checks.

- Input: `domain` (required, string)
- Zone resolution: same as Req-3 — resolve domain to zone_id via cache
- API calls (parallel where possible):
  - Zone details: already available from the zones list (status, name_servers, plan)
  - SSL settings: `GET /zones/{zone_id}/settings/ssl` → `value` field (off/flexible/full/strict)
  - Security level: `GET /zones/{zone_id}/settings/security_level` → `value` field
- Format output as structured text:

```
Domain: example.com
Status: active
Plan: Pro
Nameservers: aria.ns.cloudflare.com, bob.ns.cloudflare.com
SSL Mode: full (strict)
Security Level: medium
```

- If any settings API call fails (e.g., insufficient permissions), show the field as `unknown`
  rather than failing the entire tool

### Req-5: Module Structure and Registration

Follow the established tool module pattern:

- New file: `crates/nv-daemon/src/cloudflare_tools.rs`
- Expose `cloudflare_tool_definitions() -> Vec<ToolDefinition>` for registration
- Expose public async handler functions: `cf_zones()`, `cf_dns_records(domain, record_type)`,
  `cf_domain_status(domain)`
- Add `mod cloudflare_tools;` to `main.rs`
- Add `use crate::cloudflare_tools;` to `tools.rs`
- Call `tools.extend(cloudflare_tools::cloudflare_tool_definitions())` in `register_tools()`
- Add dispatch arms in both `execute_tool()` and `execute_tool_send()` for all three tools
- Add Cloudflare entries to `humanize_tool()` in `orchestrator.rs`:
  `"cf_zones" | "cf_dns_records" | "cf_domain_status" => "Checking Cloudflare DNS..."`

## Scope
- **IN**: three read-only Cloudflare tools, zone cache, domain-to-zone-id resolution, tool registration, humanize_tool entries
- **OUT**: DNS record creation/modification/deletion (write operations), WAF/firewall rules, Workers/Pages management, cache purge, Cloudflare analytics, config struct in `nv-core` (env var is sufficient for a single token — no TOML config needed)

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/cloudflare_tools.rs` | New file — client struct, zone cache, three tool handlers, tool definitions |
| `crates/nv-daemon/src/main.rs` | Add `mod cloudflare_tools;` declaration |
| `crates/nv-daemon/src/tools.rs` | Add `use crate::cloudflare_tools;`, extend `register_tools()`, add dispatch arms in `execute_tool()` and `execute_tool_send()` |
| `crates/nv-daemon/src/orchestrator.rs` | Add Cloudflare entries to `humanize_tool()` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Zone cache returns stale data after Cloudflare changes | 1-hour TTL is acceptable for zone list (zones change very rarely). DNS records are not cached — always fetched fresh. |
| `CLOUDFLARE_API_TOKEN` has insufficient permissions | Clear error message on 403 responses: "Token lacks Zone:Read or DNS:Read permission" |
| Cloudflare API rate limits (1200 req/10min for most endpoints) | Read-only tools with zone caching means very low request volume — not a practical concern |
| Domain name not found in zone list | Return helpful error listing all available zones so the user can correct the input |
| Settings endpoints return unexpected formats | Graceful degradation — show `unknown` for individual fields rather than failing the entire tool |
