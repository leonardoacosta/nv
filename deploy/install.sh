#!/usr/bin/env bash
set -euo pipefail

# NV Install Script
# Thin wrapper — delegates to the TypeScript daemon deployment pipeline.
# Idempotent -- safe to re-run after code changes.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SERVICE_DIR="${HOME}/.config/systemd/user"
NV_DIR="${HOME}/.nv"
CONFIG_DIR="${HOME}/.config/nv"

echo "==> Stopping running relay services..."
systemctl --user stop nv-discord-relay.service nv-teams-relay.service 2>/dev/null || true

echo "==> Creating NV directories..."
mkdir -p "$NV_DIR"/{state,memory,logs}
mkdir -p "$CONFIG_DIR"

# Create Claude CLI sandbox — minimal ~/.claude with only auth credentials.
# Prevents loading hooks, CLAUDE.md, agents, MCP servers from host config.
SANDBOX_DIR="$NV_DIR/claude-sandbox/.claude"
mkdir -p "$SANDBOX_DIR"
if [ -f "$HOME/.claude/.credentials.json" ]; then
    ln -sf "$HOME/.claude/.credentials.json" "$SANDBOX_DIR/.credentials.json"
    echo '{}' > "$SANDBOX_DIR/settings.json"
    echo "    Claude sandbox: $SANDBOX_DIR (auth-only)"
fi

# Copy example configs if no real configs exist
if [ ! -f "$PROJECT_DIR/config/env" ]; then
    cp "$PROJECT_DIR/config/env.example" "$PROJECT_DIR/config/env"
    echo "    Created config/env from example -- edit with your tokens"
fi

if [ ! -f "$PROJECT_DIR/config/nv.toml" ]; then
    cp "$PROJECT_DIR/config/nv.example.toml" "$PROJECT_DIR/config/nv.toml" 2>/dev/null || true
    echo "    Created config/nv.toml from example -- edit with your values"
fi

# Symlink config files (all source of truth lives in repo)
echo "==> Linking config files..."
ln -sf "$PROJECT_DIR/config/env" "$NV_DIR/env"
ln -sf "$PROJECT_DIR/config/nv.toml" "$NV_DIR/nv.toml"
ln -sf "$PROJECT_DIR/config/system-prompt.md" "$NV_DIR/system-prompt.md"
ln -sf "$PROJECT_DIR/config/soul.md" "$NV_DIR/soul.md"
ln -sf "$PROJECT_DIR/config/identity.md" "$NV_DIR/identity.md"
ln -sf "$PROJECT_DIR/config/user.md" "$NV_DIR/user.md"

# Bootstrap is copied (not symlinked) — it's a template consumed once
if [ ! -f "$NV_DIR/bootstrap-state.json" ]; then
    cp "$PROJECT_DIR/config/bootstrap.md" "$NV_DIR/bootstrap.md"
    echo "    Copied bootstrap.md (first-run template)"
fi

# ── Python Agent SDK Sidecar ─────────────────────────────────────────

echo "==> Installing claude-agent-sdk (Python Agent SDK)..."
pipx install claude-agent-sdk 2>/dev/null || \
  pip3 install claude-agent-sdk --break-system-packages 2>/dev/null || \
  { echo "    WARNING: Failed to install claude-agent-sdk — sidecar will not function"; }

# ── Discord Relay Bot ────────────────────────────────────────────────

echo "==> Setting up Discord relay..."
DISCORD_VENV="$NV_DIR/relays/discord/venv"
mkdir -p "$NV_DIR/relays/discord"

if [ ! -d "$DISCORD_VENV" ]; then
    python3 -m venv "$DISCORD_VENV"
    echo "    Created venv: $DISCORD_VENV"
fi

"$DISCORD_VENV/bin/pip" install -q -r "$PROJECT_DIR/relays/discord/requirements.txt"
echo "    Discord relay dependencies installed"

cp "$PROJECT_DIR/relays/discord/nv-discord-relay.service" "$SERVICE_DIR/"
echo "    Discord relay service installed"

# ── Teams Webhook Relay ──────────────────────────────────────────────

echo "==> Setting up Teams webhook relay..."
cp "$PROJECT_DIR/relays/teams/nv-teams-relay.service" "$SERVICE_DIR/"
echo "    Teams relay service installed (port ${TEAMS_WEBHOOK_PORT:-8401})"

# ── systemd Relay Services ───────────────────────────────────────────

echo "==> Configuring relay services..."
systemctl --user daemon-reload

# Enable relays if tokens are configured
if grep -q "DISCORD_BOT_TOKEN" "$NV_DIR/env" 2>/dev/null; then
    systemctl --user enable --now nv-discord-relay.service
    echo "    Discord relay: enabled (token found)"
else
    systemctl --user disable nv-discord-relay.service 2>/dev/null || true
    echo "    Discord relay: skipped (add DISCORD_BOT_TOKEN to ~/.nv/env)"
fi

if grep -q "TEAMS_WEBHOOK_SECRET" "$NV_DIR/env" 2>/dev/null || [ -n "${TEAMS_WEBHOOK_SECRET:-}" ]; then
    systemctl --user enable --now nv-teams-relay.service
    echo "    Teams relay: enabled (TEAMS_WEBHOOK_SECRET found)"
else
    systemctl --user disable nv-teams-relay.service 2>/dev/null || true
    echo "    Teams relay: skipped (add TEAMS_WEBHOOK_SECRET to ~/.nv/env to enable)"
fi

# ── Delegate to TypeScript daemon install ───────────────────────────

echo ""
echo "==> Delegating to TypeScript daemon install..."
bash "$SCRIPT_DIR/install-ts.sh"

# ── Verify ───────────────────────────────────────────────────────────

echo "==> Verifying..."

ACTIVE=$(systemctl --user is-active nova-ts.service 2>/dev/null || true)
if [ "$ACTIVE" = "active" ]; then
    echo "    nova-ts.service: active"
else
    echo "    nova-ts.service: $ACTIVE (expected 'active')"
    echo "    Check logs: journalctl --user -u nova-ts.service -n 50"
    exit 1
fi

DISCORD_ACTIVE=$(systemctl --user is-active nv-discord-relay.service 2>/dev/null || true)
echo "    discord relay: $DISCORD_ACTIVE"

TEAMS_ACTIVE=$(systemctl --user is-active nv-teams-relay.service 2>/dev/null || true)
echo "    teams relay: $TEAMS_ACTIVE"

echo ""
echo "NV installed successfully."
echo "  Config:     $NV_DIR/nv.toml"
echo "  Services:   nova-ts.service, nv-discord-relay.service, nv-teams-relay.service"
echo "  Logs:       journalctl --user -u nova-ts.service -f"
echo ""
echo "  Power Automate → POST http://$(hostname):${TEAMS_WEBHOOK_PORT:-8401}/"
echo ""
echo "  Missing tokens? Add to ~/.nv/env:"
echo "    DISCORD_BOT_TOKEN=..."
echo "    TELEGRAM_CHAT_ID=..."
