# Implementation Tasks

<!-- beads:epic:nv-uzaa -->

## Phase 1: systemd Unit File

- [x] [1.1] Create `deploy/nova-ts.service` — `Type=simple`, `ExecStart=doppler run --project nova --config prd -- node %h/.local/lib/nova-ts/dist/index.js`, `Environment=NODE_ENV=production`, `Environment=ANTHROPIC_BASE_URL=https://ai-gateway.vercel.sh`, `Restart=on-failure`, `RestartSec=5`, `TimeoutStopSec=30`, `WantedBy=default.target` [owner:api-engineer]

## Phase 2: Build and Install Script

- [x] [2.1] Create `deploy/install-ts.sh` — `set -euo pipefail`; run `npm ci` in `packages/db` and `packages/daemon`; run `npm run build` in `packages/daemon`; create `~/.local/lib/nova-ts/`; copy `packages/daemon/dist/` to `~/.local/lib/nova-ts/dist/`; copy `package.json` and `package-lock.json`; install `node_modules` via `npm ci --omit=dev` in destination [owner:api-engineer]
- [x] [2.2] Add systemd install steps to `deploy/install-ts.sh` — copy `deploy/nova-ts.service` to `~/.config/systemd/user/nova-ts.service`; `systemctl --user daemon-reload`; `systemctl --user enable nova-ts.service`; `systemctl --user restart nova-ts.service` (depends: 2.1, 1.1) [owner:api-engineer]
- [x] [2.3] Add health check verification to `deploy/install-ts.sh` — sleep 5 seconds after restart; assert `systemctl --user is-active nova-ts.service` exits 0; assert `curl -sf http://localhost:8400/health` returns 200; on failure, print `journalctl --user -u nova-ts.service -n 20` and exit 1; on success, print summary with version and status (depends: 2.2) [owner:api-engineer]
- [x] [2.4] Make `deploy/install-ts.sh` executable — `chmod +x deploy/install-ts.sh`; verify shebang is `#!/usr/bin/env bash` [owner:api-engineer]

---

## Validation Gates

| Phase | Gate |
|-------|------|
| 1 Unit file | `systemd-analyze verify deploy/nova-ts.service` reports no errors |
| 2 Install script | `bash -n deploy/install-ts.sh` passes (syntax check); script is executable |
| **Final** | `deploy/install-ts.sh` completes successfully; `systemctl --user is-active nova-ts.service` returns `active`; `curl localhost:8400/health` returns HTTP 200 |
