# Implementation Tasks

<!-- beads:epic:pending -->

## DB Batch

(no database changes)

## API Batch

- [x] [2.1] [P-1] Rewrite `deploy/install.sh` to a thin wrapper that sources common vars then calls `install-ts.sh` — remove all `cargo build`, binary copy, `chmod +x`, `NV_HEALTH_PORT` health check, `nv status` reference, and Rust summary lines. Keep Discord relay, Teams relay, Claude sandbox, config symlinks, and systemd relay logic intact. Update health check to verify `nova-ts.service` is active. [owner:api-engineer]
- [x] [2.2] [P-1] Rewrite `deploy/nv.service` to run the TypeScript daemon — change `ExecStart` to `doppler run --project nova --config prd -- node %h/.local/lib/nova-ts/packages/daemon/dist/index.js`, change `Type` from `notify` to `simple`, remove `NotifyAccess=all`, `WatchdogSec=120`, `Environment=RUST_LOG=info`, add `Environment=NODE_ENV=production`, `WorkingDirectory=%h/.local/lib/nova-ts`, update `PATH` to include `%h/.local/share/pnpm` [owner:api-engineer]
- [x] [2.3] [P-1] Remove `/target/` entry from `.gitignore` (Rust build output directory no longer relevant) [owner:api-engineer]
- [x] [2.4] [P-1] Remove migration note block from `deploy/install-ts.sh` — delete the "Migration note" lines that reference stopping/disabling the Rust daemon via `systemctl --user stop nv.service` and `systemctl --user disable nv.service` [owner:api-engineer]
- [x] [2.5] [P-2] Grep the repository for remaining references to `cargo build`, `cargo install`, `cargo test`, `nv-daemon`, `nv-cli`, `crates/`, `target/release`, `RUST_LOG` (excluding `openspec/changes/archive/`) — remove or update any active references found in scripts, config, or documentation [owner:api-engineer]

## UI Batch

(no UI changes)

## E2E Batch

- [ ] [4.1] [P-2] Verify deploy scripts are syntactically valid — run `bash -n deploy/install.sh` and `bash -n deploy/install-ts.sh`, confirm no parse errors [owner:e2e-engineer]
- [ ] [4.2] [P-2] Verify no dangling Rust references remain — grep for `nv-daemon`, `cargo build`, `RUST_LOG`, `target/release` across the repo (excluding `openspec/changes/archive/`), confirm zero matches in active code [owner:e2e-engineer]
