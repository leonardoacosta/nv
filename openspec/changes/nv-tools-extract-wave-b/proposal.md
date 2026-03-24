# Proposal: Extract HTTP Tools Wave B

## Change ID
`nv-tools-extract-wave-b`

## Summary

Move 7 stateless HTTP tool files (ha, ado, plaid, doppler, cloudflare, posthog, neon) from
nv-daemon to nv-tools. Same pattern as wave-a. `neon.rs` additionally needs `tokio-postgres`.

## Context
- Depends on: `nv-tools-extract-wave-a`
- `neon.rs` uses `tokio-postgres` + `rustls` for direct DB queries alongside reqwest for API
- All other files are pure reqwest patterns

## What Changes

Same per-tool pattern as wave-a: move file, update imports, re-export, register.
`neon.rs` additionally needs `tokio-postgres`, `tokio-postgres-rustls`, `rustls` in nv-tools deps.

## Scope
- **IN**: 7 tool files (ha, ado, plaid, doppler, cloudflare, posthog, neon)
- **OUT**: Process tools, daemon-coupled tools

## Impact
| Area | Change |
|------|--------|
| `crates/nv-tools/Cargo.toml` | Add tokio-postgres + rustls deps for neon |
| 7 tool .rs files | Moved from daemon to nv-tools |
| `crates/nv-daemon/src/tools/mod.rs` | 7 more re-exports |
