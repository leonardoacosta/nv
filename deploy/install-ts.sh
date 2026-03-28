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
# HEALTH_PORT removed — slim daemon (v10) has no HTTP server

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

# Run pending Drizzle migrations against the live database.
# Uses the same Doppler project/config as the systemd service.
echo "==> Running database migrations..."
(cd "${PROJECT_DIR}/packages/db" && doppler run --project nova --config prd -- pnpm db:migrate)

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

# Copy config/ directory (system prompt, identity, soul, user context, toml)
# The daemon resolves config/system-prompt.md relative to its working directory
# (INSTALL_DIR).  Without this copy the daemon silently skips the system prompt.
echo "    Copying config/..."
mkdir -p "${INSTALL_DIR}/config"
cp "${PROJECT_DIR}/config/system-prompt.md" "${INSTALL_DIR}/config/"
cp "${PROJECT_DIR}/config/identity.md"      "${INSTALL_DIR}/config/"
cp "${PROJECT_DIR}/config/soul.md"          "${INSTALL_DIR}/config/"
cp "${PROJECT_DIR}/config/user.md"          "${INSTALL_DIR}/config/"
cp "${PROJECT_DIR}/config/bootstrap.md"     "${INSTALL_DIR}/config/"
cp "${PROJECT_DIR}/config/nv.toml"          "${INSTALL_DIR}/config/"
# Copy contact/ sub-directory if present
if [ -d "${PROJECT_DIR}/config/contact" ]; then
    cp -r "${PROJECT_DIR}/config/contact/." "${INSTALL_DIR}/config/contact/"
fi

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

# Note: slim-daemon (v10) removed the Hono API server. The daemon no longer
# exposes an HTTP health endpoint. systemctl is-active (above) is sufficient.
# Tool fleet health is checked by install-tools.sh via tool-router :4100/health.
echo "    nova-ts.service: active (systemd)"

# ── Tool Fleet Deploy ────────────────────────────────────────────────────────

TOOLS_SCRIPT="${SCRIPT_DIR}/install-tools.sh"
TOOLS_STATUS="skipped"

if [ -f "$TOOLS_SCRIPT" ]; then
    echo ""
    echo "==> Deploying tool fleet..."
    if bash "$TOOLS_SCRIPT"; then
        TOOLS_STATUS="success"
    else
        TOOLS_STATUS="failed (exit $?)"
        echo ""
        echo "WARNING: Tool fleet deploy failed. Daemon is running normally."
        echo "  Re-run: bash ${TOOLS_SCRIPT}"
    fi
else
    echo ""
    echo "==> Skipping tool fleet deploy (install-tools.sh not found)"
fi

# ── Summary ──────────────────────────────────────────────────────────────────

VERSION=$(node -e "const p=require('${INSTALL_DIR}/packages/daemon/package.json'); process.stdout.write(p.version)" 2>/dev/null || echo "unknown")

echo ""
echo "nova-ts installed successfully."
echo "  Version:  ${VERSION}"
echo "  Install:  ${INSTALL_DIR}"
echo "  Service:  nova-ts.service (active)"
echo "  Health:   systemctl --user is-active nova-ts.service"
echo "  Tools:    ${TOOLS_STATUS}"
echo "  Logs:     journalctl --user -u nova-ts.service -f"
echo ""
echo "Migration note:"
echo "  To stop the Rust daemon:    systemctl --user stop nv.service"
echo "  To disable the Rust daemon: systemctl --user disable nv.service"
echo ""
