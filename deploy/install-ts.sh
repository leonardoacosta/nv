#!/usr/bin/env bash
set -euo pipefail

# Nova TypeScript Daemon Install Script
# Builds from source, installs to ~/.local/lib/nova-ts/, and configures the
# systemd user service.  Idempotent -- safe to re-run after code changes.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="${HOME}/.local/lib/nova-ts"
SERVICE_DIR="${HOME}/.config/systemd/user"
HEALTH_PORT="${NV_DAEMON_PORT:-7700}"

echo "==> Building packages..."

# Build packages/db (shared schema / client used by daemon)
echo "    Building packages/db..."
pnpm --filter @nova/db build

# Build packages/daemon (compiles TypeScript to dist/)
echo "    Building packages/daemon..."
pnpm --filter @nova/daemon build

echo "==> Installing to ${INSTALL_DIR}..."

# Create destination directory
mkdir -p "${INSTALL_DIR}/dist"

# Copy compiled output
cp -r "${PROJECT_DIR}/packages/daemon/dist/." "${INSTALL_DIR}/dist/"

# Copy package manifest so npm/pnpm can resolve prod deps in place
cp "${PROJECT_DIR}/packages/daemon/package.json" "${INSTALL_DIR}/package.json"

# Install production-only node_modules at the deploy target.
# This produces a self-contained directory that can be run with plain node.
echo "    Installing production dependencies..."
pnpm install --prod --ignore-workspace --prefix "${INSTALL_DIR}"

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

VERSION=$(node -e "const p=require('${INSTALL_DIR}/package.json'); process.stdout.write(p.version)" 2>/dev/null || echo "unknown")

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
