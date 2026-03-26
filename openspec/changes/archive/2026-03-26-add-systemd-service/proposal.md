# Proposal: Deploy TS Daemon as systemd User Service

## Change ID
`add-systemd-service`

## Summary

Deploy the TypeScript daemon as a systemd user service on the homelab, replacing the Rust `nv.service`. Adds a build/install script, a production-ready unit file with Doppler secret injection, and a health check verification step.

## Context

- Phase: 1 — Foundation | Wave: 3
- Depends on: `scaffold-ts-daemon` (epic `nv-c7v9`) — requires a built TS daemon with `packages/daemon` and `packages/db`
- Replaces: `deploy/nv.service` (Rust daemon unit) — the Rust daemon is stopped, not deleted, allowing coexistence during cutover
- Related: existing `deploy/install.sh` (Rust install script) — a separate `deploy/install-ts.sh` is added alongside it

## Motivation

The Nova project is migrating from a Rust daemon to a TypeScript daemon. The TS daemon brings faster iteration, shared types with the Next.js dashboard, and direct use of the Claude Agent SDK. To run persistently on the homelab it needs a production-ready systemd user service that:

1. Starts automatically on boot and restarts on failure
2. Injects secrets via Doppler (no hardcoded keys)
3. Exposes a health check endpoint confirming it started cleanly
4. Provides a clear migration path from the Rust daemon

## Requirements

### Req-1: Build and Install Script

Create `deploy/install-ts.sh` — idempotent build and deploy script:

1. `npm ci` in `packages/daemon` and `packages/db`
2. `npm run build` (tsc) in `packages/daemon`
3. `mkdir -p ~/.local/lib/nova-ts/`
4. Copy compiled output from `packages/daemon/dist/` to `~/.local/lib/nova-ts/dist/`
5. Copy `package.json`, `package-lock.json`, and `node_modules/` (or symlink) so the installed path is self-contained
6. Copy `deploy/nova-ts.service` to `~/.config/systemd/user/nova-ts.service`
7. `systemctl --user daemon-reload`
8. `systemctl --user enable nova-ts.service`
9. `systemctl --user restart nova-ts.service`
10. Wait 5 seconds, then verify:
    - `systemctl --user is-active nova-ts.service` exits 0
    - `curl -sf http://localhost:8400/health` returns HTTP 200
11. Print pass/fail summary with systemd status on failure

The script uses `set -euo pipefail`. It is safe to re-run after code changes (idempotent via `restart`).

### Req-2: systemd Unit File

Create `deploy/nova-ts.service`:

```ini
[Unit]
Description=Nova TypeScript Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=doppler run --project nova --config prd -- node %h/.local/lib/nova-ts/dist/index.js
Environment=NODE_ENV=production
Environment=ANTHROPIC_BASE_URL=https://ai-gateway.vercel.sh
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

[Install]
WantedBy=default.target
```

Key design choices:
- `Type=simple` — Node.js process is the main process; no sd-notify needed in v1
- `ExecStart` runs via `doppler run` so all secrets are injected as environment variables at runtime — no `.env` file on disk
- `ANTHROPIC_BASE_URL` set to Vercel AI Gateway to route API calls through the homelab gateway
- `Restart=on-failure` with `RestartSec=5` matches the Rust daemon's restart behavior
- Service name `nova-ts` is distinct from the Rust `nv.service` — both can coexist during migration

### Req-3: Migration Strategy

The cutover procedure is manual and documented in the install script output:

1. Stop the Rust daemon: `systemctl --user stop nv.service`
2. Run `deploy/install-ts.sh` to build and start `nova-ts.service`
3. Verify health: `curl localhost:8400/health`
4. Both `nv.service` and `nova-ts.service` can coexist if they run on different ports
5. Once verified stable, optionally disable the Rust daemon: `systemctl --user disable nv.service`

The install script does NOT stop `nv.service` automatically — the operator controls the cutover timing.

### Req-4: Health Check Verification

The install script verifies the daemon is healthy after startup:

- `curl -sf http://localhost:8400/health` — expects HTTP 200
- If this fails, the script prints the systemd journal tail (`journalctl --user -u nova-ts.service -n 20`) and exits non-zero

The `/health` endpoint itself is part of the `scaffold-ts-daemon` spec — this spec only calls it.

## Out of Scope

- Implementing the `/health` endpoint (that belongs to `scaffold-ts-daemon`)
- Automatic Rust daemon cutover (manual operator decision)
- Log rotation (Node.js stdout goes to journald via systemd; no additional rotation needed in v1)
- Watchdog / sd-notify integration (deferred to a hardening spec)
- CI/CD pipeline triggering the deploy (manual `deploy/install-ts.sh` for now)
- Multi-environment support (single homelab target for v1)
