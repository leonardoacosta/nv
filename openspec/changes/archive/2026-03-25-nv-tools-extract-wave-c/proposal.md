# Proposal: Extract Process-Shelling Tools

## Change ID
`nv-tools-extract-wave-c`

## Summary

Move 5 process-shelling/utility tool files (docker, github, web, calendar, tailscale) from
nv-daemon to nv-tools. These tools use `Command::new()` or reqwest with additional deps.

## Context
- Depends on: `nv-tools-extract-wave-b`
- `docker.rs` and `github.rs` shell out to `docker`/`gh` CLI
- `web.rs` uses reqwest for URL fetching + HTML extraction
- `calendar.rs` uses reqwest + `base64` + `chrono` for Google Calendar JWT auth
- `tailscale.rs` lives at `crates/nv-daemon/src/tailscale.rs` (not in tools/) -- may need
  wrapping via SharedDeps instead of direct move if it has daemon-specific state

## What Changes

Same move pattern. `calendar.rs` needs `base64` + `jsonwebtoken` added to nv-tools deps.
`tailscale.rs` needs investigation -- if it's purely a `Command::new("tailscale")` wrapper,
it moves cleanly; if it has daemon state, defer to SharedDeps spec.

## Scope
- **IN**: docker, github, web, calendar, tailscale (if clean)
- **OUT**: Daemon-coupled tools, jira module, teams

## Impact
| Area | Change |
|------|--------|
| `crates/nv-tools/Cargo.toml` | Add base64, jsonwebtoken for calendar |
| 5 tool .rs files | Moved from daemon to nv-tools |
| `crates/nv-daemon/src/tools/mod.rs` | 5 more re-exports |
