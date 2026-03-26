# Proposal: Add Fleet Deploy Infrastructure

## Change ID
`add-fleet-deploy`

## Summary
Create the systemd and deployment infrastructure for the Nova tool fleet: a `nova-tools.target` grouping 9 individual `.service` files (one per tool microservice), a deploy script that builds all tool packages and installs them alongside the existing daemon, and fleet management commands (start/stop/restart all tools via the target).

## Context
- Extends: `deploy/install-ts.sh`, `deploy/nova-ts.service`, `deploy/pre-push.sh`
- Depends on: `scaffold-tool-service` (Wave 1) — assumes `packages/tools/{service}/` directories exist with `package.json` and `src/index.ts` for each of the 9 services
- Related: `add-tool-router` (Wave 2, same wave — router is one of the 9 services being deployed)
- Architecture: `docs/plan/nova-v10/scope-lock.md` defines the fleet layout

## Motivation
The v10 tool fleet splits 47 tools across 9 independently deployable Hono microservices. Each needs its own systemd user service for independent restart, crash recovery, and log isolation. Without a target group and deploy script, managing 9 services individually is untenable. The existing `install-ts.sh` only handles the daemon and db packages — it needs to build and install the full tool fleet alongside them.

The deploy script must produce a self-contained install directory at `~/.local/lib/nova-ts/packages/tools/` that mirrors the mini-workspace pattern already established for the daemon, so pnpm workspace references resolve correctly at runtime.

## Requirements

### Req-1: systemd target group
A `nova-tools.target` that groups all 9 tool services. `systemctl --user start nova-tools.target` starts all tools. `systemctl --user stop nova-tools.target` stops all tools. Individual services can still be managed independently.

### Req-2: Individual service files
One `.service` file per tool service, following the existing `nova-ts.service` pattern:
- `ExecStart=doppler run --project nova --config prd -- node {install_path}/dist/index.js`
- `PartOf=nova-tools.target` (so target stop cascades)
- `WantedBy=nova-tools.target` (so target start cascades)
- `Environment=NODE_ENV=production`
- `Environment=PATH=%h/.local/bin:%h/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin`
- `Restart=on-failure`, `RestartSec=5`, `TimeoutStopSec=15`

Services and ports:
| Service | Unit Name | Port | Install Path |
|---------|-----------|------|-------------|
| tool-router | nova-tool-router.service | 4000 | packages/tools/tool-router |
| memory-svc | nova-memory-svc.service | 4001 | packages/tools/memory-svc |
| messages-svc | nova-messages-svc.service | 4002 | packages/tools/messages-svc |
| channels-svc | nova-channels-svc.service | 4003 | packages/tools/channels-svc |
| discord-svc | nova-discord-svc.service | 4004 | packages/tools/discord-svc |
| teams-svc | nova-teams-svc.service | 4005 | packages/tools/teams-svc |
| schedule-svc | nova-schedule-svc.service | 4006 | packages/tools/schedule-svc |
| graph-svc | nova-graph-svc.service | 4007 | packages/tools/graph-svc |
| meta-svc | nova-meta-svc.service | 4008 | packages/tools/meta-svc |

Each service sets `Environment=PORT=<port>` so the Hono server binds correctly.

### Req-3: Deploy script
`deploy/install-tools.sh` — builds all 9 tool packages and installs to `~/.local/lib/nova-ts/packages/tools/`. Steps:
1. Build each `@nova/tool-*` package via pnpm filter
2. Copy dist + package.json for each service into install dir
3. Update the workspace root `pnpm-workspace.yaml` to include `packages/tools/*`
4. Install production deps (`pnpm install --prod` from install root)
5. Copy all `.service` files + target to `~/.config/systemd/user/`
6. `systemctl --user daemon-reload`
7. Enable and start `nova-tools.target`
8. Health check each service (HTTP GET to its port)
9. Print summary

The script is idempotent — safe to re-run after code changes.

### Req-4: Update existing deploy infrastructure
- `deploy/install-ts.sh`: Add a call to `deploy/install-tools.sh` after daemon install, so a single `install-ts.sh` run deploys everything
- `deploy/pre-push.sh`: Add tool fleet deployment after daemon deploy, with health verification

### Req-5: Fleet management convenience
Document fleet management commands in the deploy script output:
- `systemctl --user start nova-tools.target` — start all tools
- `systemctl --user stop nova-tools.target` — stop all tools
- `systemctl --user restart nova-tools.target` — restart all tools
- `systemctl --user status nova-tools.target` — fleet status
- `journalctl --user -u nova-memory-svc.service -f` — per-service logs

## Scope
- **IN**: systemd target + 9 service files, install-tools.sh script, update install-ts.sh and pre-push.sh, health checks
- **OUT**: Traefik routing (separate spec), MCP registration (register-mcp-servers spec), individual tool service code (handled by per-service specs), Docker packaging

## Impact
| Area | Change |
|------|--------|
| `deploy/nova-tools.target` | New — systemd target grouping all tool services |
| `deploy/nova-*.service` | New — 9 service unit files |
| `deploy/install-tools.sh` | New — build + install + health check script |
| `deploy/install-ts.sh` | Modified — call install-tools.sh at end |
| `deploy/pre-push.sh` | Modified — add fleet deploy step |

## Risks
| Risk | Mitigation |
|------|-----------|
| Services not yet implemented when deploy is merged | install-tools.sh is gated — skips services whose source dir doesn't exist, logs which were skipped |
| Port conflicts | Each service has a fixed port assignment per scope-lock; script validates no port collisions during health check |
| Doppler project/config not provisioned | Script checks `doppler run` exit code before enabling service; skips with warning if Doppler fails |
| pnpm install fails in install dir | Clear and recreate install dir (same pattern as existing install-ts.sh) |

## Dependencies
- `scaffold-tool-service` — service directories must exist for the build step (but script gracefully skips missing ones)
