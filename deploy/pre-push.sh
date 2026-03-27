#!/usr/bin/env bash
# Nova - Pre-push deploy hook
#
# Selectively deploys only the components that changed:
#   - Daemon + tool fleet: when packages/daemon/, packages/db/, packages/tools/, deploy/, or config/ change
#   - Dashboard: when apps/dashboard/ or packages/db/ change
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
    CHANGED_FILES="(first push — all files)"
    DAEMON_CHANGED=true
    DASHBOARD_CHANGED=true
else
    CHANGED_FILES=$(git diff --name-only "$REMOTE_SHA"..HEAD 2>/dev/null || echo "")

    # Daemon/fleet: packages/daemon/, packages/db/, packages/tools/, deploy/, config/
    DAEMON_CHANGED=false
    if echo "$CHANGED_FILES" | grep -qE '^(packages/daemon/|packages/db/|packages/tools/|deploy/|config/)'; then
        DAEMON_CHANGED=true
    fi

    # Dashboard: apps/dashboard/, packages/db/ (shared dep), docker-compose.yml, .dockerignore
    DASHBOARD_CHANGED=false
    if echo "$CHANGED_FILES" | grep -qE '^(apps/dashboard/|packages/db/|docker-compose\.yml|\.dockerignore)'; then
        DASHBOARD_CHANGED=true
    fi
fi

# ── Summary ────────────────────────────────────────────────────────────────

if ! $DAEMON_CHANGED && ! $DASHBOARD_CHANGED; then
    echo "=== Nova: No deployable changes (docs/specs/config only) ==="
    echo "    Skipping deploy. Push continues."
    exit 0
fi

echo "=== Nova: Selective deploy ==="
echo "    Daemon/fleet: $($DAEMON_CHANGED && echo 'YES — changed' || echo 'skip')"
echo "    Dashboard:    $($DASHBOARD_CHANGED && echo 'YES — changed' || echo 'skip')"
echo "    Log: ${LOG_FILE}"
echo ""

RESULTS=()

# ── Deploy Daemon + Tool Fleet ─────────────────────────────────────────────

if $DAEMON_CHANGED; then
    DEPLOY_SCRIPT="${GIT_ROOT}/deploy/install-ts.sh"

    if [[ ! -f "$DEPLOY_SCRIPT" ]]; then
        echo "Warning: deploy/install-ts.sh not found — skipping daemon deploy"
        RESULTS+=("daemon: SKIPPED (no script)")
    elif bash "$DEPLOY_SCRIPT" 2>&1 | tee "$LOG_FILE"; then
        echo ""
        echo "Daemon + tool fleet deploy succeeded."
        RESULTS+=("daemon+fleet: OK")
    else
        echo ""
        echo "Daemon deploy failed. Push continues — check logs:"
        echo "  ${LOG_FILE}"
        echo "  journalctl --user -u nova-ts.service -n 30"
        RESULTS+=("daemon+fleet: FAILED")
    fi
else
    RESULTS+=("daemon+fleet: skip (no changes)")
fi

# ── Deploy Dashboard ───────────────────────────────────────────────────────

if $DASHBOARD_CHANGED; then
    COMPOSE_FILE="${GIT_ROOT}/docker-compose.yml"

    if [[ ! -f "$COMPOSE_FILE" ]]; then
        echo "    No docker-compose.yml found — skipping dashboard deploy"
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
                echo "    Dashboard container not running."
                RESULTS+=("dashboard: FAILED (not running)")
            fi
        else
            echo "    Dashboard build failed."
            RESULTS+=("dashboard: FAILED (build)")
        fi
    fi
else
    RESULTS+=("dashboard: skip (no changes)")
fi

# ── Summary + TTS ──────────────────────────────────────────────────────────

echo ""
echo "=== Deploy Summary ==="
for r in "${RESULTS[@]}"; do
    echo "    $r"
done

# Build TTS message from results
TTS_MSG="Nova deploy:"
for r in "${RESULTS[@]}"; do
    TTS_MSG="$TTS_MSG $r."
done
_notify "$TTS_MSG"

exit 0
