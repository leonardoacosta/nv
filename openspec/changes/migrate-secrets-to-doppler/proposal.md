# Proposal: Migrate Secrets to Doppler

## Change ID
`migrate-secrets-to-doppler`

## Summary
Replace the flat `~/.nv/env` secrets file with Doppler as the single source of truth for all
API keys, tokens, and credentials across the daemon, CLI, and relay services.

## Context
- Extends: `deploy/nv.service`, `relays/teams/nv-teams-relay.service`, `relays/discord/nv-discord-relay.service`, `crates/nv-cli/src/main.rs`
- Related: All other projects (oo, tc, tl, mv, ss) already use Doppler for secrets management

## Motivation
NV currently stores ~30 secrets in a flat file (`~/.nv/env`) loaded by systemd's
`EnvironmentFile` directive and manually parsed by the CLI. This lacks rotation, audit trails,
and access control. Every other project in the ecosystem uses Doppler — NV is the holdout.

Structured configuration (`nv.toml`) stays as-is. It handles non-secret config (agent model,
chat IDs, nexus topology, daemon settings) well and is version-controllable. The problem is
exclusively the secrets file.

## Requirements

### Req-1: Doppler project setup
Create a Doppler project `nova` with a single `prd` environment containing all secrets currently
in `config/env`, including homelab-specific vars (`HA_TOKEN`, `HA_URL`, `PLAID_DB_URL`).

### Req-2: Systemd service migration
All three systemd services (`nv.service`, `nv-teams-relay.service`, `nv-discord-relay.service`)
must use `doppler run --` to inject secrets instead of `EnvironmentFile`. The `--fallback=true`
flag ensures the daemon survives Doppler API outages by falling back to cached secrets.

### Req-3: CLI secret injection
The CLI's manual env file parser (`main.rs:63-85`) must be removed. CLI invocations will be
wrapped with `doppler run -- nv <command>` to receive secrets from Doppler.

### Req-4: Repo configuration
A `doppler.yaml` must be added to the repo root to declare the project/config mapping, enabling
`doppler run` without explicit `--project`/`--config` flags from the repo directory.

### Req-5: Legacy cleanup
`config/env` is removed from the deployment flow. `config/env.example` is updated to document
the Doppler migration and serve as a reference for which secrets exist.

## Scope
- **IN**: Doppler project creation, systemd service updates, CLI env parser removal, `doppler.yaml`, env.example update
- **OUT**: Changes to `nv.toml` or `Config` struct, changes to `Secrets::from_env()` or tool `from_env()` constructors, Doppler SDK integration in Rust code

## Impact
| Area | Change |
|------|--------|
| `deploy/nv.service` | `ExecStart` wrapped with `doppler run --fallback=true --`, `EnvironmentFile` removed |
| `relays/teams/nv-teams-relay.service` | Same pattern |
| `relays/discord/nv-discord-relay.service` | Same pattern |
| `crates/nv-cli/src/main.rs` | Remove lines 63-85 (manual env file parser) |
| `config/env.example` | Update with Doppler migration note |
| `doppler.yaml` (new) | Doppler project/config declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| Doppler CLI not installed on homelab | Pre-flight check in task list; `doppler --version` gate |
| Doppler API outage prevents daemon start | `--fallback=true` uses locally cached secrets |
| Service token not configured for systemd | Explicit `[user]` task for token setup |
| Relay services (Python) need Doppler too | Same `doppler run --` pattern works for any process |
