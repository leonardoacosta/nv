#!/usr/bin/env bash
set -euo pipefail

# Nova Tool Fleet Install Script
# Builds all tool packages, installs to ~/.local/lib/nova-ts/packages/tools/,
# configures systemd user services, and verifies health.
# Idempotent -- safe to re-run after code changes.
#
# Install layout:
#
#   ~/.local/lib/nova-ts/
#   ├── packages/
#   │   ├── daemon/     (installed by install-ts.sh)
#   │   ├── db/         (shared dependency)
#   │   └── tools/
#   │       ├── tool-router/
#   │       │   ├── dist/
#   │       │   └── package.json
#   │       ├── memory-svc/
#   │       │   ├── dist/
#   │       │   └── package.json
#   │       └── ... (8 more services)
#   └── pnpm-workspace.yaml

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="${HOME}/.local/lib/nova-ts"
SERVICE_DIR="${HOME}/.config/systemd/user"

# ── Service definitions ──────────────────────────────────────────────────────

SERVICES=(
  tool-router
  memory-svc
  messages-svc
  channels-svc
  discord-svc
  teams-svc
  schedule-svc
  graph-svc
  meta-svc
)

PORTS=(
  4000
  4001
  4002
  4003
  4004
  4005
  4006
  4007
  4008
)

# ── Pre-flight checks ───────────────────────────────────────────────────────

echo "==> [tools] Running pre-flight checks..."

if ! command -v pnpm &> /dev/null; then
    echo "ERROR: 'pnpm' not found in PATH."
    exit 1
fi
echo "    pnpm $(pnpm --version) -- OK"

DOPPLER_AVAILABLE=true
if ! command -v doppler &> /dev/null; then
    echo "    WARNING: 'doppler' not found -- services may fail to start without secrets"
    DOPPLER_AVAILABLE=false
else
    echo "    doppler $(doppler --version 2>/dev/null | head -1) -- OK"
fi

# ── Build ────────────────────────────────────────────────────────────────────

echo "==> [tools] Building packages..."

# Build shared db dependency first
echo "    Building @nova/db..."
(cd "$PROJECT_DIR" && pnpm --filter @nova/db build)

BUILT=()
SKIPPED=()

for svc in "${SERVICES[@]}"; do
    SVC_DIR="${PROJECT_DIR}/packages/tools/${svc}"
    if [ ! -d "$SVC_DIR" ]; then
        echo "    Skipping ${svc} -- source not found at ${SVC_DIR}"
        SKIPPED+=("$svc")
        continue
    fi

    if [ ! -f "${SVC_DIR}/package.json" ]; then
        echo "    Skipping ${svc} -- no package.json"
        SKIPPED+=("$svc")
        continue
    fi

    echo "    Building @nova/${svc}..."
    (cd "$PROJECT_DIR" && pnpm --filter "@nova/${svc}" build)
    BUILT+=("$svc")
done

echo "    Built: ${#BUILT[@]}, Skipped: ${#SKIPPED[@]}"

if [ ${#BUILT[@]} -eq 0 ]; then
    echo "WARNING: No tool services were built. Nothing to install."
    exit 0
fi

# ── Install ──────────────────────────────────────────────────────────────────

echo "==> [tools] Installing to ${INSTALL_DIR}/packages/tools/..."

for svc in "${BUILT[@]}"; do
    SVC_SRC="${PROJECT_DIR}/packages/tools/${svc}"
    SVC_DST="${INSTALL_DIR}/packages/tools/${svc}"

    # Clean previous install for this service
    rm -rf "$SVC_DST"
    mkdir -p "$SVC_DST"

    # Copy dist and package.json
    if [ -d "${SVC_SRC}/dist" ]; then
        cp -r "${SVC_SRC}/dist/." "${SVC_DST}/dist/"
    fi
    cp "${SVC_SRC}/package.json" "${SVC_DST}/package.json"
done

# Update workspace yaml to include tools
cat > "${INSTALL_DIR}/pnpm-workspace.yaml" <<'EOF'
packages:
  - "packages/*"
  - "packages/tools/*"
EOF

# Install production deps from workspace root
echo "    Installing production dependencies..."
(cd "${INSTALL_DIR}" && pnpm install --prod)

# ── systemd ──────────────────────────────────────────────────────────────────

echo "==> [tools] Installing systemd services..."

mkdir -p "${SERVICE_DIR}"

# Copy target file
cp "${SCRIPT_DIR}/nova-tools.target" "${SERVICE_DIR}/nova-tools.target"

# Copy service files -- only for built services
ENABLED=()
for svc in "${BUILT[@]}"; do
    UNIT_FILE="nova-${svc}.service"
    if [ -f "${SCRIPT_DIR}/${UNIT_FILE}" ]; then
        cp "${SCRIPT_DIR}/${UNIT_FILE}" "${SERVICE_DIR}/${UNIT_FILE}"
        ENABLED+=("$svc")
    else
        echo "    WARNING: ${UNIT_FILE} not found in deploy/ -- skipping"
    fi
done

systemctl --user daemon-reload

# Enable the target
systemctl --user enable nova-tools.target 2>/dev/null || true

# Enable individual services that were built
for svc in "${ENABLED[@]}"; do
    systemctl --user enable "nova-${svc}.service" 2>/dev/null || true
done

# Restart the target (cascades to all PartOf services)
echo "    Restarting nova-tools.target..."
systemctl --user restart nova-tools.target 2>/dev/null || true

# ── Health check ─────────────────────────────────────────────────────────────

echo "==> [tools] Running health checks (waiting 5s for startup)..."
sleep 5

HEALTH_PASS=0
HEALTH_FAIL=0

echo ""
printf "    %-20s %-12s %-10s\n" "SERVICE" "SYSTEMD" "HTTP"
printf "    %-20s %-12s %-10s\n" "-------" "-------" "----"

for i in "${!SERVICES[@]}"; do
    svc="${SERVICES[$i]}"
    port="${PORTS[$i]}"

    # Check if this service was built
    BUILT_MATCH=false
    for b in "${BUILT[@]}"; do
        if [ "$b" = "$svc" ]; then
            BUILT_MATCH=true
            break
        fi
    done

    if ! $BUILT_MATCH; then
        printf "    %-20s %-12s %-10s\n" "$svc" "skipped" "skipped"
        continue
    fi

    # Check systemd status
    ACTIVE=$(systemctl --user is-active "nova-${svc}.service" 2>/dev/null || echo "inactive")

    # Check HTTP health
    HTTP_STATUS="fail"
    if curl -sf "http://127.0.0.1:${port}/health" > /dev/null 2>&1; then
        HTTP_STATUS="pass"
    fi

    if [ "$ACTIVE" = "active" ] && [ "$HTTP_STATUS" = "pass" ]; then
        printf "    %-20s %-12s %-10s\n" "$svc" "active" "pass"
        HEALTH_PASS=$((HEALTH_PASS + 1))
    else
        printf "    %-20s %-12s %-10s\n" "$svc" "$ACTIVE" "$HTTP_STATUS"
        HEALTH_FAIL=$((HEALTH_FAIL + 1))
    fi
done

echo ""

# ── MCP registration ────────────────────────────────────────────────────────

REGISTER_SCRIPT="${PROJECT_DIR}/scripts/register-mcp-servers.sh"
if [ -f "$REGISTER_SCRIPT" ]; then
    echo "==> [tools] Registering MCP servers in ~/.claude/mcp.json..."
    if REGISTER_OUTPUT=$(bash "$REGISTER_SCRIPT" 2>&1); then
        echo "$REGISTER_OUTPUT" | sed 's/^/    /'
    else
        echo "    WARNING: MCP registration failed (exit $?) — non-fatal"
        echo "$REGISTER_OUTPUT" | sed 's/^/    /'
    fi
else
    echo "==> [tools] Skipping MCP registration — script not found at ${REGISTER_SCRIPT}"
fi

# ── Summary ──────────────────────────────────────────────────────────────────

echo "==> [tools] Fleet install complete."
echo "  Installed: ${#BUILT[@]} services"
echo "  Skipped:   ${#SKIPPED[@]} services"
echo "  Health:    ${HEALTH_PASS} pass, ${HEALTH_FAIL} fail"
echo ""
echo "  Fleet management:"
echo "    systemctl --user start nova-tools.target    -- start all tools"
echo "    systemctl --user stop nova-tools.target     -- stop all tools"
echo "    systemctl --user restart nova-tools.target  -- restart all tools"
echo "    systemctl --user status nova-tools.target   -- fleet status"
echo "    journalctl --user -u nova-<service>.service -f  -- per-service logs"
echo ""

# Exit 0 even if some services failed health checks -- they may not have tool
# code yet (Wave 1 only scaffolded the template). Warn loudly instead.
if [ "$HEALTH_FAIL" -gt 0 ]; then
    echo "  WARNING: ${HEALTH_FAIL} service(s) did not pass health checks."
    echo "  This is expected if tool code has not been implemented yet."
    echo "  Check logs: journalctl --user -u nova-<service>.service -n 30"
fi

exit 0
