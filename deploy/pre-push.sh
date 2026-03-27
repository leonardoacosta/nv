#!/usr/bin/env bash
# Nova - Pre-push deploy hook (graceful)
#
# Selectively deploys only the components that changed. Avoids restarting
# the daemon when Nova may be mid-conversation — only restarts when the
# daemon code, DB schema, config, or MCP registration changed.
#
# Skip deployment:  SKIP_DEPLOY=1 git push
#
# Hook setup (run once after clone):
#   ln -sf ../../deploy/pre-push.sh .git/hooks/pre-push

set -euo pipefail

LOG_FILE="/tmp/nova-deploy.log"

_notify() {
    local message="$1"
    echo "{\"message\":\"${message}\"}" | ~/.claude/scripts/bin/claude-notify --notify 2>/dev/null || true
}

# Skip deployment if requested
if [[ "${SKIP_DEPLOY:-0}" == "1" ]]; then
    echo "SKIP_DEPLOY=1: Skipping deployment"
    exit 0
fi

# Only deploy on pushes to main
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
    exit 0
fi

GIT_ROOT=$(git rev-parse --show-toplevel)

# ── Detect what changed ────────────────────────────────────────────────────
# Compare local HEAD to what's on origin/main (the commits being pushed)

REMOTE_SHA=$(git rev-parse origin/main 2>/dev/null || echo "")
if [[ -z "$REMOTE_SHA" ]]; then
    # First push — deploy everything
    CHANGED_FILES="(first push)"
    DAEMON_CHANGED=true
    FLEET_CHANGED=true
    DASHBOARD_CHANGED=true
else
    CHANGED_FILES=$(git diff --name-only "$REMOTE_SHA"..HEAD 2>/dev/null || echo "")

    # Daemon restart needed: daemon code, DB schema, config, MCP, systemd services
    # These require the daemon process to reload to pick up changes.
    DAEMON_CHANGED=false
    if echo "$CHANGED_FILES" | grep -qE '^(packages/daemon/|packages/db/|config/|deploy/.*\.service|deploy/.*\.target)'; then
        DAEMON_CHANGED=true
    fi

    # Fleet rebuild needed: tool service code or shared DB package
    # Individual services restart independently — doesn't affect the daemon.
    FLEET_CHANGED=false
    if echo "$CHANGED_FILES" | grep -qE '^(packages/tools/|packages/db/)'; then
        FLEET_CHANGED=true
    fi

    # Dashboard rebuild: dashboard code, shared DB, docker config
    DASHBOARD_CHANGED=false
    if echo "$CHANGED_FILES" | grep -qE '^(apps/dashboard/|packages/db/|docker-compose\.yml|\.dockerignore)'; then
        DASHBOARD_CHANGED=true
    fi
fi

# ── Summary ────────────────────────────────────────────────────────────────

if ! $DAEMON_CHANGED && ! $FLEET_CHANGED && ! $DASHBOARD_CHANGED; then
    echo "=== Nova: No deployable changes (docs/specs only) ==="
    echo "    Skipping deploy. Push continues."
    exit 0
fi

echo "=== Nova: Selective deploy ==="
echo "    Daemon:    $($DAEMON_CHANGED && echo 'YES — will restart' || echo 'skip (Nova keeps running)')"
echo "    Fleet:     $($FLEET_CHANGED && echo 'YES — services restart' || echo 'skip')"
echo "    Dashboard: $($DASHBOARD_CHANGED && echo 'YES — container rebuild' || echo 'skip')"
echo "    Log: ${LOG_FILE}"
echo ""

RESULTS=()

# ── Deploy Daemon ──────────────────────────────────────────────────────────
# Only when daemon code/config/schema changed. Avoids interrupting Nova
# mid-conversation for fleet-only or dashboard-only changes.

if $DAEMON_CHANGED; then
    DEPLOY_SCRIPT="${GIT_ROOT}/deploy/install-ts.sh"

    if [[ ! -f "$DEPLOY_SCRIPT" ]]; then
        echo "Warning: deploy/install-ts.sh not found — skipping daemon deploy"
        RESULTS+=("daemon: SKIPPED (no script)")
    else
        # install-ts.sh also calls install-tools.sh internally
        if bash "$DEPLOY_SCRIPT" 2>&1 | tee "$LOG_FILE"; then
            echo ""
            echo "Daemon + fleet deploy succeeded."
            RESULTS+=("daemon: OK (restarted)")
            RESULTS+=("fleet: OK (via daemon deploy)")
            FLEET_CHANGED=false  # Already handled by install-ts.sh
        else
            echo ""
            echo "Daemon deploy failed. Push continues — check logs."
            RESULTS+=("daemon: FAILED")
        fi
    fi
else
    RESULTS+=("daemon: skip (not interrupted)")
fi

# ── Deploy Fleet Only ──────────────────────────────────────────────────────
# When only tool services changed — rebuild and restart fleet without
# touching the daemon. Nova stays up processing messages.

if $FLEET_CHANGED; then
    TOOLS_SCRIPT="${GIT_ROOT}/deploy/install-tools.sh"

    if [[ ! -f "$TOOLS_SCRIPT" ]]; then
        echo "Warning: deploy/install-tools.sh not found — skipping fleet deploy"
        RESULTS+=("fleet: SKIPPED (no script)")
    else
        echo ""
        echo "=== Nova: Deploying tool fleet only (daemon stays up) ==="
        if bash "$TOOLS_SCRIPT" 2>&1 | tee -a "$LOG_FILE"; then
            echo ""
            echo "Tool fleet deploy succeeded. Daemon was NOT restarted."
            RESULTS+=("fleet: OK (daemon untouched)")
        else
            echo ""
            echo "Fleet deploy failed. Daemon still running."
            RESULTS+=("fleet: FAILED")
        fi
    fi
fi

# ── Deploy Dashboard ───────────────────────────────────────────────────────

if $DASHBOARD_CHANGED; then
    COMPOSE_FILE="${GIT_ROOT}/docker-compose.yml"

    if [[ ! -f "$COMPOSE_FILE" ]]; then
        RESULTS+=("dashboard: SKIPPED (no compose)")
    else
        echo ""
        echo "=== Nova: Rebuilding dashboard container ==="

        if docker compose -f "$COMPOSE_FILE" build --no-cache dashboard 2>&1 | tee -a "$LOG_FILE"; then
            docker compose -f "$COMPOSE_FILE" up -d dashboard 2>&1 | tee -a "$LOG_FILE"

            echo "    Waiting for dashboard to start..."
            sleep 5

            DASH_HEALTHY=false
            for i in 1 2 3; do
                if docker compose -f "$COMPOSE_FILE" ps dashboard --format json 2>/dev/null | grep -q '"running"'; then
                    DASH_HEALTHY=true
                    break
                fi
                sleep 3
            done

            if $DASH_HEALTHY; then
                echo "    Dashboard deploy succeeded."
                RESULTS+=("dashboard: OK")
            else
                RESULTS+=("dashboard: FAILED (not running)")
            fi
        else
            RESULTS+=("dashboard: FAILED (build)")
        fi
    fi
else
    RESULTS+=("dashboard: skip (no changes)")
fi

# ── Post-Deploy Health Check ──────────────────────────────────────────────

echo ""
echo "=== Post-Deploy Health Check ==="

# Wait for services to stabilize after restart
sleep 8

# Check fleet health via nv CLI (if available)
NV_CLI="${HOME}/dev/nv/packages/cli/dist/index.js"
if [[ -f "$NV_CLI" ]]; then
    NV_STATUS=$(node "$NV_CLI" status 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' || echo "nv status: unavailable")
    echo "    $NV_STATUS"
    RESULTS+=("health: $NV_STATUS")
else
    # Fallback: manual fleet probe
    FLEET_OK=0
    FLEET_TOTAL=0
    for port in 4100 4101 4102 4103 4104 4105 4106 4107 4108 4109; do
        FLEET_TOTAL=$((FLEET_TOTAL + 1))
        curl -sf --max-time 2 "http://127.0.0.1:${port}/health" > /dev/null 2>&1 && FLEET_OK=$((FLEET_OK + 1))
    done
    echo "    Fleet: ${FLEET_OK}/${FLEET_TOTAL} healthy"
    RESULTS+=("health: fleet ${FLEET_OK}/${FLEET_TOTAL}")
fi

# ── Summary + TTS ──────────────────────────────────────────────────────────

echo ""
echo "=== Deploy Summary ==="
for r in "${RESULTS[@]}"; do
    echo "    $r"
done

TTS_MSG="Nova deploy:"
for r in "${RESULTS[@]}"; do
    TTS_MSG="$TTS_MSG $r."
done
_notify "$TTS_MSG"

exit 0
