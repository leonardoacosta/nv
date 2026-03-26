#!/usr/bin/env bash
set -euo pipefail

# Nova TypeScript Daemon Install Script
# Builds from source, installs to ~/.local/lib/nova-ts/, and configures the
# systemd user service.  Idempotent -- safe to re-run after code changes.
#
# Install layout (mini pnpm workspace so workspace:* deps resolve correctly):
#
#   ~/.local/lib/nova-ts/
#   ├── package.json           (private workspace root, no deps)
#   ├── pnpm-workspace.yaml    (packages: ["packages/*"])
#   └── packages/
#       ├── daemon/
#       │   ├── dist/
#       │   └── package.json
#       └── db/
#           ├── dist/
#           └── package.json

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="${HOME}/.local/lib/nova-ts"
SERVICE_DIR="${HOME}/.config/systemd/user"
HEALTH_PORT="${NV_DAEMON_PORT:-7700}"

# ── Pre-flight checks ────────────────────────────────────────────────────────

echo "==> Running pre-flight checks..."

# The Agent SDK spawns `claude` as a subprocess — verify the binary is present.
if ! command -v claude &> /dev/null; then
    echo ""
    echo "ERROR: 'claude' binary not found in PATH."
    echo "  Expected: ${HOME}/.local/bin/claude"
    echo "  Install Claude Code, then re-run this script."
    exit 1
fi
echo "    claude $(claude --version 2>/dev/null | head -1) -- OK"

# The Agent SDK reads credentials from ~/.claude/ -- verify the directory exists.
if [ ! -d "${HOME}/.claude" ]; then
    echo ""
    echo "ERROR: ${HOME}/.claude/ directory not found."
    echo "  The Agent SDK needs Claude credentials to authenticate."
    echo "  Run 'claude' interactively at least once to complete auth, then re-run."
    exit 1
fi
echo "    ${HOME}/.claude/ credentials directory -- OK"

echo "==> Building packages..."

# Build packages/db (shared schema / client used by daemon)
echo "    Building packages/db..."
pnpm --filter @nova/db build

# Build packages/daemon (compiles TypeScript to dist/)
echo "    Building packages/daemon..."
pnpm --filter @nova/daemon build

echo "==> Installing to ${INSTALL_DIR}..."

# Remove any prior install so pnpm does not encounter a stale node_modules
# layout from a different workspace structure (avoids ERR_PNPM_ABORTED_REMOVE_MODULES_DIR).
rm -rf "${INSTALL_DIR}"

# Create mini workspace directory structure
mkdir -p "${INSTALL_DIR}/packages/daemon"
mkdir -p "${INSTALL_DIR}/packages/db"

# Write workspace root package.json (private, no deps — just anchors the workspace)
cat > "${INSTALL_DIR}/package.json" <<'EOF'
{
  "name": "nova-ts-install",
  "version": "0.0.0",
  "private": true
}
EOF

# Write pnpm-workspace.yaml so pnpm resolves workspace:* references
cat > "${INSTALL_DIR}/pnpm-workspace.yaml" <<'EOF'
packages:
  - "packages/*"
EOF

# Copy daemon dist + manifest
cp -r "${PROJECT_DIR}/packages/daemon/dist/." "${INSTALL_DIR}/packages/daemon/dist/"
cp "${PROJECT_DIR}/packages/daemon/package.json" "${INSTALL_DIR}/packages/daemon/package.json"

# Copy db dist + manifest
cp -r "${PROJECT_DIR}/packages/db/dist/." "${INSTALL_DIR}/packages/db/dist/"
cp "${PROJECT_DIR}/packages/db/package.json" "${INSTALL_DIR}/packages/db/package.json"

# Install production-only node_modules from the workspace root so pnpm can
# resolve @nova/db@workspace:* correctly.  Must cd into the install dir —
# using --prefix keeps pnpm anchored to the monorepo workspace and causes it
# to see the wrong set of packages.
echo "    Installing production dependencies..."
(cd "${INSTALL_DIR}" && pnpm install --prod)

echo "==> Installing systemd service..."

mkdir -p "${SERVICE_DIR}"
cp "${SCRIPT_DIR}/nova-ts.service" "${SERVICE_DIR}/nova-ts.service"

systemctl --user daemon-reload
systemctl --user enable nova-ts.service
systemctl --user restart nova-ts.service

echo "==> Waiting for service to start..."
sleep 5

# ── Health Check ─────────────────────────────────────────────────────────────

ACTIVE=$(systemctl --user is-active nova-ts.service 2>/dev/null || true)
if [ "${ACTIVE}" != "active" ]; then
    echo ""
    echo "ERROR: nova-ts.service is not active (state: ${ACTIVE})"
    echo ""
    echo "--- Journal (last 20 lines) ---"
    journalctl --user -u nova-ts.service -n 20 --no-pager || true
    exit 1
fi

if ! curl -sf "http://127.0.0.1:${HEALTH_PORT}/health" > /dev/null 2>&1; then
    echo ""
    echo "ERROR: Health endpoint did not respond at http://127.0.0.1:${HEALTH_PORT}/health"
    echo ""
    echo "--- Journal (last 20 lines) ---"
    journalctl --user -u nova-ts.service -n 20 --no-pager || true
    exit 1
fi

# ── Summary ──────────────────────────────────────────────────────────────────

VERSION=$(node -e "const p=require('${INSTALL_DIR}/packages/daemon/package.json'); process.stdout.write(p.version)" 2>/dev/null || echo "unknown")

echo ""
echo "nova-ts installed successfully."
echo "  Version:  ${VERSION}"
echo "  Install:  ${INSTALL_DIR}"
echo "  Service:  nova-ts.service (active)"
echo "  Health:   http://127.0.0.1:${HEALTH_PORT}/health"
echo "  Logs:     journalctl --user -u nova-ts.service -f"
echo ""
echo "Migration note:"
echo "  To stop the Rust daemon:    systemctl --user stop nv.service"
echo "  To disable the Rust daemon: systemctl --user disable nv.service"
echo ""
