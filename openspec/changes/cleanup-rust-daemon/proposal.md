# Proposal: Cleanup Rust Daemon References

## Change ID
`cleanup-rust-daemon`

## Summary
Remove all Rust daemon references from deploy scripts, systemd units, and documentation after crates/ deletion. Migrate `deploy/install.sh` and `deploy/nv.service` to reference the TypeScript daemon exclusively.

## Context
- The Rust crates (`nv-daemon`, `nv-cli`, `nv-core`, `nv-tools`, `ado-cli`) in `crates/` have been deleted
- `deploy/install.sh` still references `cargo build --release -p nv-daemon -p nv-cli`, binary installation to `~/.local/bin/`, and health checks against the `nv-daemon` binary
- `deploy/nv.service` still runs `nv-daemon` binary via doppler with `Type=notify`, `WatchdogSec`, and `RUST_LOG` environment
- The TypeScript daemon at `packages/daemon/` is the sole runtime now
- TS daemon runs via: `node packages/daemon/dist/index.js` (or tsx in dev)
- `deploy/install-ts.sh` and `deploy/nova-ts.service` already exist as the correct TS deployment infrastructure
- `deploy/install-ts.sh` still prints a migration note referencing the Rust daemon (`systemctl --user stop nv.service`)

## Motivation
Dead references to deleted Rust binaries will break deployments. `deploy/install.sh` will fail at `cargo build` since the `crates/` directory no longer exists. `deploy/nv.service` references a binary (`nv-daemon`) that cannot be built. The `.gitignore` still excludes `/target/` (the Rust build output directory) which is no longer relevant. The TS deploy script still prints migration notes about stopping the Rust daemon, implying a dual-runtime setup that no longer exists. All of these should be cleaned up so only the TypeScript daemon path remains.

## Requirements

### Req-1: Rewrite deploy/install.sh
Rewrite `deploy/install.sh` to delegate to the TypeScript deployment pipeline. Options:
- **Option A (recommended):** Make `install.sh` a thin wrapper that calls `install-ts.sh`, so any existing scripts or documentation referencing `install.sh` continue to work
- **Option B:** Delete `install.sh` entirely and update any references to point at `install-ts.sh`

In either case, remove:
- `cargo build --release -p nv-daemon -p nv-cli`
- Binary copy to `~/.local/bin/nv-daemon` and `~/.local/bin/nv`
- `chmod +x` for Rust binaries
- Health check against `NV_HEALTH_PORT` / `http://127.0.0.1:${HEALTH_PORT}/health`
- Summary line referencing `$INSTALL_DIR/nv-daemon, $INSTALL_DIR/nv`
- The `nv status` reference (Rust CLI command)

Keep:
- Discord relay setup (Python venv, pip install, service copy) -- unchanged
- Teams webhook relay setup (service copy) -- unchanged
- Claude sandbox directory creation -- unchanged
- Config symlinks and directory creation -- unchanged
- systemd daemon-reload and relay enable/disable logic -- unchanged

Update health check to verify `nova-ts.service` is active (matching the pattern in `install-ts.sh`).

### Req-2: Rewrite deploy/nv.service
Replace the Rust daemon unit with a TypeScript daemon unit:
- Change `ExecStart` from `%h/.local/bin/doppler run ... -- %h/.local/bin/nv-daemon` to `doppler run --project nova --config prd -- node %h/.local/lib/nova-ts/packages/daemon/dist/index.js`
- Change `Type` from `notify` to `simple` (Node.js does not implement sd_notify)
- Remove `NotifyAccess=all` and `WatchdogSec=120` (notify-specific directives)
- Remove `Environment=RUST_LOG=info`
- Add `Environment=NODE_ENV=production`
- Add `WorkingDirectory=%h/.local/lib/nova-ts`
- Update `PATH` to include `%h/.local/share/pnpm` (needed for pnpm workspace resolution)

Alternatively, since `deploy/nova-ts.service` already contains the correct unit definition, `nv.service` could be deleted and all references updated to use `nova-ts.service`. The decision depends on whether any external scripts or cron jobs reference `nv.service` by name.

### Req-3: Clean up .gitignore
Remove the `/target/` entry (Rust build output directory). The `crates/` directory no longer exists, so Cargo will never produce a `target/` directory in this repo.

### Req-4: Remove migration notes from install-ts.sh
The summary section of `deploy/install-ts.sh` (lines 185-188) prints:
```
Migration note:
  To stop the Rust daemon:    systemctl --user stop nv.service
  To disable the Rust daemon: systemctl --user disable nv.service
```
Remove this block. The migration is complete.

### Req-5: Verify no remaining Rust references
Grep the repository for remaining references to:
- `cargo build`, `cargo install`, `cargo test`
- `nv-daemon` (the Rust binary name)
- `nv-cli` (the Rust CLI binary)
- `crates/`
- `target/release`
- `RUST_LOG`

Exclude `openspec/changes/archive/` from the search (archived proposals are historical records). Flag any remaining references in active code, scripts, or configuration for removal.

## Scope
- **IN**: `deploy/install.sh`, `deploy/nv.service`, `.gitignore`, `deploy/install-ts.sh` (migration note removal)
- **OUT**: All TypeScript code (unchanged), tool fleet services (unchanged), `deploy/nova-ts.service` (already correct), `deploy/install-tools.sh` (already correct), relay services (Discord/Teams -- unchanged), archived OpenSpec proposals

## Impact
| Area | Change |
|------|--------|
| `deploy/install.sh` | Rewrite -- remove cargo build, delegate to TS pipeline or replace entirely |
| `deploy/nv.service` | Rewrite -- node instead of nv-daemon binary, or delete in favor of nova-ts.service |
| `.gitignore` | Remove `/target/` entry |
| `deploy/install-ts.sh` | Remove migration note block (4 lines) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Deploy breaks during transition | `install-ts.sh` is already tested and deployed. The change removes a broken path (`install.sh`), not a working one. |
| External scripts reference `nv.service` by name | Grep for `nv.service` outside of deploy/ to find any cron jobs, monitoring scripts, or documentation that reference it. Update those references to `nova-ts.service`. |
| systemd unit changes require daemon restart | Run `systemctl --user daemon-reload` after installing the updated unit. Coordinate restart via `systemctl --user restart nova-ts.service`. |
| Relay services depend on install.sh flow | Relay setup (Discord/Teams) is independent of the daemon binary. The relay sections of install.sh can be preserved or extracted to a separate script. |
