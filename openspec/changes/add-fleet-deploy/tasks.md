# Implementation Tasks

## Phase 1: systemd Target and Service Files

- [x] [1.1] Create `deploy/nova-tools.target` — systemd target unit: `Description=Nova Tool Fleet`, `Wants=` listing all 9 service names, `After=network-online.target`. No `[Install]` section — target is started by `install-tools.sh` enabling it. [owner:devops-engineer]
- [x] [1.2] Create service file template and generate all 9 service files in `deploy/`: `nova-tool-router.service` (PORT=4000), `nova-memory-svc.service` (PORT=4001), `nova-messages-svc.service` (PORT=4002), `nova-channels-svc.service` (PORT=4003), `nova-discord-svc.service` (PORT=4004), `nova-teams-svc.service` (PORT=4005), `nova-schedule-svc.service` (PORT=4006), `nova-graph-svc.service` (PORT=4007), `nova-meta-svc.service` (PORT=4008). Each: `Type=simple`, `WorkingDirectory=%h/.local/lib/nova-ts`, `ExecStart=doppler run --project nova --config prd -- node %h/.local/lib/nova-ts/packages/tools/{name}/dist/index.js`, `Environment=NODE_ENV=production PORT={port}`, `Environment=PATH=%h/.local/bin:%h/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin`, `Environment=HOME=%h`, `PartOf=nova-tools.target`, `WantedBy=nova-tools.target`, `Restart=on-failure`, `RestartSec=5`, `TimeoutStopSec=15` [owner:devops-engineer]

## Phase 2: Tool Fleet Deploy Script

- [x] [2.1] Create `deploy/install-tools.sh` with pre-flight section: verify `pnpm` available, verify `doppler` available (warn if missing), define `INSTALL_DIR=~/.local/lib/nova-ts`, `SERVICE_DIR=~/.config/systemd/user`, define the 9 service names and ports as parallel arrays [owner:devops-engineer]
- [x] [2.2] Add build section to `install-tools.sh`: iterate over the 9 tool service directories in `packages/tools/`, skip any that don't exist yet (log "Skipping {name} — source not found"), build each present service via `pnpm --filter @nova/{name} build`. Also build `@nova/db` as a shared dependency. [owner:devops-engineer]
- [x] [2.3] Add install section to `install-tools.sh`: for each built service, create `${INSTALL_DIR}/packages/tools/{name}/`, copy `dist/` and `package.json`. Update `${INSTALL_DIR}/pnpm-workspace.yaml` to include `packages/tools/*`. Run `pnpm install --prod` from `${INSTALL_DIR}`. [owner:devops-engineer]
- [x] [2.4] Add systemd section to `install-tools.sh`: copy `nova-tools.target` + all `nova-*.service` files to `${SERVICE_DIR}`, run `systemctl --user daemon-reload`, `systemctl --user enable nova-tools.target`, only enable individual services whose source was built (skip others). Restart the target. [owner:devops-engineer]
- [x] [2.5] Add health check section to `install-tools.sh`: wait 5s after restart, then for each enabled service, check `systemctl --user is-active` and `curl -sf http://127.0.0.1:{port}/health`. Print per-service pass/fail table. Script exits 0 even if some services fail (they may not have tool code yet), but warns loudly. [owner:devops-engineer]
- [x] [2.6] Add summary section to `install-tools.sh`: print installed count, skipped count, health pass/fail, and fleet management commands (start/stop/restart/status/logs). Make the script executable (`chmod +x`). [owner:devops-engineer]

## Phase 3: Update Existing Deploy Infrastructure

- [x] [3.1] Update `deploy/install-ts.sh`: after the daemon health check (line ~149), add a section that calls `bash "${SCRIPT_DIR}/install-tools.sh"` if the script exists. Capture exit code but don't fail the daemon install if tool deploy fails — warn instead. Update the final summary to include tool fleet status. [owner:devops-engineer]
- [x] [3.2] Update `deploy/pre-push.sh`: after the daemon deploy section (line ~62), add a tool fleet deploy section. Call `install-tools.sh` with output piped to the log file. Update TTS notifications to include fleet status (e.g. "Nova deploy succeeded — daemon + tools + dashboard"). [owner:devops-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 systemd | All `.service` files and `.target` file parse correctly: `systemd-analyze verify deploy/nova-*.service deploy/nova-tools.target` (or manual inspection for correct syntax) |
| 2 Script | `bash -n deploy/install-tools.sh` passes (syntax check). Script runs on homelab with at least one tool service present, installs and starts it. |
| 3 Integration | `deploy/install-ts.sh` runs end-to-end, deploying daemon + tool fleet. `deploy/pre-push.sh` syntax check passes. |
| **Final** | `systemctl --user start nova-tools.target` starts all available tool services. `systemctl --user stop nova-tools.target` stops them all. Individual services restart independently. |
