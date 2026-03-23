# Proposal: Doppler Secrets Management Tools

## Change ID
`add-doppler-tools`

## Summary

Add three read-only Doppler tools to the Nova daemon that let the agent inspect secret inventory
across projects and environments without ever exposing secret values. Uses the Doppler REST API v3
with `DOPPLER_API_TOKEN` env var for authentication, and a `[doppler]` config section in `nv.toml`
for project alias mapping.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool registration and dispatch), `crates/nv-core/src/config.rs` (Config struct)
- New file: `crates/nv-daemon/src/doppler_tools.rs` (client, definitions, formatters)
- Pattern: follows the same module pattern as `vercel_tools.rs` — client struct with `from_env()`, public tool functions, `doppler_tool_definitions()` for registration
- Auth: `DOPPLER_API_TOKEN` env var (Doppler personal token or service token)
- Depends on: nothing — standalone addition

## Motivation

Nova manages deployments across multiple projects that all use Doppler for secrets. When debugging
environment issues, missing env vars, or dev/prod configuration drift, the operator currently has
to leave the conversation and use the Doppler CLI or dashboard. These tools let Nova answer
questions like "which secrets does oo have in dev but not prod?" or "who last changed secrets in
tc?" directly in the chat — without ever revealing secret values.

## Requirements

### Req-1: DopplerConfig in `nv.toml`

Add an optional `[doppler]` section to the config with a project alias map. This allows the
operator to use short codes (e.g. `oo`) instead of full Doppler project names.

```toml
[doppler]
[doppler.projects]
oo = "otaku-odyssey"
tc = "tribal-cities"
tl = "tavern-ledger"
mv = "modern-visa"
ss = "styles-silas"
```

- Add `DopplerConfig` struct to `crates/nv-core/src/config.rs`
- Add `pub doppler: Option<DopplerConfig>` to the `Config` struct
- `DopplerConfig` contains `projects: HashMap<String, String>` (alias -> Doppler project name)

### Req-2: `doppler_secrets` Tool

List secret **names only** for a Doppler project and environment. Never return values.

- API: `GET https://api.doppler.com/v3/configs/config/secrets` with query params `project` and `config` (Doppler's term for environment)
- Auth: Bearer token from `DOPPLER_API_TOKEN` env var
- Input params:
  - `project` (string, required) — project alias or full Doppler project name
  - `environment` (string, required) — Doppler config name (e.g. `dev`, `stg`, `prd`, `dev_e2e`)
- Output: sorted list of secret names with count, formatted as a readable string
- CRITICAL: extract only the keys from the API response object — discard all values, computed values, and raw values before formatting
- Resolve project aliases via `DopplerConfig.projects` if the config is present; pass through raw name if no alias match

### Req-3: `doppler_compare` Tool

Compare secret names between two environments of the same project. Shows which secrets exist in
one environment but not the other.

- Input params:
  - `project` (string, required) — project alias or full Doppler project name
  - `env_a` (string, required) — first environment (e.g. `dev`)
  - `env_b` (string, required) — second environment (e.g. `prd`)
- Implementation: call the secrets list API twice (for env_a and env_b), collect name sets, compute symmetric difference
- Output: three sections — "Only in {env_a}", "Only in {env_b}", "Common" (with count only for common)
- If both environments have identical secret names, return a "fully aligned" message

### Req-4: `doppler_activity` Tool

Fetch recent activity log entries for a project.

- API: `GET https://api.doppler.com/v3/logs` with query param `project`
- Input params:
  - `project` (string, required) — project alias or full Doppler project name
  - `count` (integer, optional) — number of entries to return (default: 10, max: 25)
- Output: formatted list of recent activity entries showing timestamp, user, action/text
- CRITICAL: if any activity entry contains secret values in its text field, redact them (though the activity API typically returns action descriptions, not values)

### Req-5: DopplerClient Struct

Create a `DopplerClient` in `doppler_tools.rs` following the `VercelClient` pattern:

- `from_env()` constructor reads `DOPPLER_API_TOKEN` env var
- Shared `reqwest::Client` with 15-second timeout
- Bearer token auth on all requests
- `map_status()` for actionable error messages (401 = token invalid, 403 = insufficient scope, 429 = rate limited)
- Project alias resolution: accept an `Option<&DopplerConfig>` in tool functions for alias lookup

### Req-6: Security Invariant

NEVER return secret values. This must be enforced at multiple levels:

- The secrets list endpoint returns full secret objects — extract **only** the key names from the response JSON object keys
- No `include_values`, `include_dynamic_secrets`, or similar params that might cause value inclusion
- Formatter functions must not have access to values — pass only `Vec<String>` of names to formatters
- Add a doc comment on every public function stating "Returns secret names only — never values"

## Scope
- **IN**: `DopplerConfig` in config, `DopplerClient` struct, three tools (`doppler_secrets`, `doppler_compare`, `doppler_activity`), tool registration in `tools.rs`, `mod doppler_tools` in `main.rs`
- **OUT**: writing/modifying secrets, secret value retrieval, Doppler project/environment management, webhook integration, syncing secrets to other services

## Impact
| Area | Change |
|------|--------|
| `crates/nv-core/src/config.rs` | Add `DopplerConfig` struct and `doppler` field on `Config` |
| `crates/nv-daemon/src/doppler_tools.rs` | New file: `DopplerClient`, three tool functions, `doppler_tool_definitions()`, formatters |
| `crates/nv-daemon/src/tools.rs` | Import `doppler_tools`, call `doppler_tool_definitions()` in `register_tools()`, add dispatch arms in `execute_tool` and `execute_tool_send` |
| `crates/nv-daemon/src/main.rs` | Add `mod doppler_tools;` |

## Risks
| Risk | Mitigation |
|------|-----------|
| Accidental secret value exposure | Extract only JSON object keys from the secrets response — values are never deserialized into any struct. Formatter functions accept `Vec<String>` of names only. Doc comments enforce the invariant. |
| `DOPPLER_API_TOKEN` not set | `DopplerClient::from_env()` returns `Err` — tool calls fail gracefully with "DOPPLER_API_TOKEN env var not set" message, same pattern as `VercelClient` |
| Project alias not found in config | Fall through to raw project name — if Doppler rejects it, the API error is surfaced. No silent failures. |
| Doppler API rate limiting | 15-second timeout + `map_status()` returns actionable 429 message. No retry logic (keep it simple). |
| Config missing `[doppler]` section | Field is `Option<DopplerConfig>` — tools still work with full project names, alias resolution is skipped |
