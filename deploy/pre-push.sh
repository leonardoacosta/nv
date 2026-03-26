#!/usr/bin/env bash
# Nova TypeScript Daemon - Pre-push deploy hook
#
# Automatically deploys the Nova TS daemon when pushing to main.
# TTS announces success or failure. Deploy failures warn but do NOT
# block the push — the code always lands.
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
DEPLOY_SCRIPT="${GIT_ROOT}/deploy/install-ts.sh"

if [[ ! -f "$DEPLOY_SCRIPT" ]]; then
    echo "Warning: deploy/install-ts.sh not found at ${DEPLOY_SCRIPT} — skipping deploy"
    exit 0
fi

echo "=== Nova: Deploying TS daemon to homelab ==="
echo "    Log: ${LOG_FILE}"
echo ""

# Run the deploy — capture output to log but also stream to terminal
if bash "$DEPLOY_SCRIPT" 2>&1 | tee "$LOG_FILE"; then
    echo ""
    echo "Deploy succeeded."
    _notify "Nova deploy succeeded"
else
    DEPLOY_EXIT="${PIPESTATUS[0]}"
    echo ""
    echo "Deploy failed (exit ${DEPLOY_EXIT}). Push continues — check logs:"
    echo "  ${LOG_FILE}"
    echo "  journalctl --user -u nova-ts.service -n 30"
    echo ""
    echo "Re-deploy manually:  bash deploy/install-ts.sh"
    echo "Skip next time:      SKIP_DEPLOY=1 git push"
    _notify "Nova deploy failed, check logs"
    # Exit 0 intentionally — do not block the push on deploy failure
fi

exit 0
