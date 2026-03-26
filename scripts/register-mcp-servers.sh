#!/usr/bin/env bash
set -euo pipefail

# Register Nova tool fleet services as MCP servers in ~/.claude/mcp.json
#
# Each service is registered as a stdio MCP server with its dist entry point
# and the --mcp flag. The script preserves all existing non-Nova entries.
#
# Usage:
#   bash scripts/register-mcp-servers.sh [--remove-legacy]
#
# Options:
#   --remove-legacy  Remove the legacy nv-tools MCP server entry
#
# Environment:
#   NOVA_INSTALL_DIR  Override install path (default: ~/.local/lib/nova-ts)

# ── Pre-flight ──────────────────────────────────────────────────────────────

if ! command -v jq &> /dev/null; then
    echo "ERROR: 'jq' is required but not found."
    echo "Install: sudo pacman -S jq  (Arch) | sudo apt install jq (Debian)"
    exit 1
fi

REMOVE_LEGACY=false
for arg in "$@"; do
    if [ "$arg" = "--remove-legacy" ]; then
        REMOVE_LEGACY=true
    fi
done

INSTALL_DIR="${NOVA_INSTALL_DIR:-${HOME}/.local/lib/nova-ts}"
MCP_JSON="${HOME}/.claude/mcp.json"

# ── Service definitions ─────────────────────────────────────────────────────

SERVICES=(
    nova-memory:memory-svc
    nova-messages:messages-svc
    nova-channels:channels-svc
    nova-discord:discord-svc
    nova-teams:teams-svc
    nova-schedule:schedule-svc
    nova-graph:graph-svc
    nova-meta:meta-svc
)

# ── Read existing config ────────────────────────────────────────────────────

if [ -f "$MCP_JSON" ]; then
    EXISTING=$(cat "$MCP_JSON")
else
    # Create the file with an empty mcpServers object
    mkdir -p "$(dirname "$MCP_JSON")"
    EXISTING='{"mcpServers":{}}'
fi

# Ensure mcpServers key exists
if ! echo "$EXISTING" | jq -e '.mcpServers' > /dev/null 2>&1; then
    EXISTING=$(echo "$EXISTING" | jq '. + {"mcpServers":{}}')
fi

# ── Add/update Nova services ────────────────────────────────────────────────

ADDED=0
UPDATED=0

RESULT="$EXISTING"

for entry in "${SERVICES[@]}"; do
    NAME="${entry%%:*}"
    SVC="${entry##*:}"
    DIST_PATH="${INSTALL_DIR}/packages/tools/${SVC}/dist/index.js"

    NEW_ENTRY=$(jq -n --arg cmd "node" --arg path "$DIST_PATH" \
        '{"command": $cmd, "args": [$path, "--mcp"]}')

    # Check if entry already exists
    if echo "$RESULT" | jq -e ".mcpServers[\"${NAME}\"]" > /dev/null 2>&1; then
        UPDATED=$((UPDATED + 1))
    else
        ADDED=$((ADDED + 1))
    fi

    RESULT=$(echo "$RESULT" | jq --arg name "$NAME" --argjson entry "$NEW_ENTRY" \
        '.mcpServers[$name] = $entry')
done

# ── Handle legacy removal ───────────────────────────────────────────────────

REMOVED=0
if $REMOVE_LEGACY; then
    if echo "$RESULT" | jq -e '.mcpServers["nv-tools"]' > /dev/null 2>&1; then
        RESULT=$(echo "$RESULT" | jq 'del(.mcpServers["nv-tools"])')
        REMOVED=1
    fi
fi

# ── Write result ────────────────────────────────────────────────────────────

echo "$RESULT" | jq '.' > "$MCP_JSON"

# ── Summary ─────────────────────────────────────────────────────────────────

KEPT=$(echo "$RESULT" | jq '.mcpServers | length')
echo "MCP server registration complete:"
echo "  Added:   ${ADDED}"
echo "  Updated: ${UPDATED}"
echo "  Removed: ${REMOVED}"
echo "  Total:   ${KEPT} servers in ${MCP_JSON}"

exit 0
