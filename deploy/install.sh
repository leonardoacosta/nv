#!/usr/bin/env bash
set -euo pipefail

# NV Install Script
# Builds from source, installs binaries, and sets up the systemd user service.
# Idempotent -- safe to re-run after code changes.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
INSTALL_DIR="${HOME}/.local/bin"
SERVICE_DIR="${HOME}/.config/systemd/user"
NV_DIR="${HOME}/.nv"
CONFIG_DIR="${HOME}/.config/nv"
HEALTH_PORT="${NV_HEALTH_PORT:-8400}"

echo "==> Building NV (release)..."
cd "$PROJECT_DIR"
cargo build --release -p nv-daemon -p nv-cli

echo "==> Installing binaries to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
cp target/release/nv-daemon "$INSTALL_DIR/nv-daemon"
cp target/release/nv-cli "$INSTALL_DIR/nv"
chmod +x "$INSTALL_DIR/nv-daemon" "$INSTALL_DIR/nv"

echo "==> Creating NV directories..."
mkdir -p "$NV_DIR"/{state,memory,logs}
mkdir -p "$CONFIG_DIR"

# Copy example config if no config exists
if [ ! -f "$NV_DIR/nv.toml" ] && [ -f "$PROJECT_DIR/config/nv.example.toml" ]; then
    cp "$PROJECT_DIR/config/nv.example.toml" "$NV_DIR/nv.toml"
    echo "    Copied nv.example.toml to $NV_DIR/nv.toml -- edit with your values"
fi

echo "==> Installing systemd user service..."
mkdir -p "$SERVICE_DIR"
cp "$SCRIPT_DIR/nv.service" "$SERVICE_DIR/nv.service"

systemctl --user daemon-reload
systemctl --user enable nv.service

echo "==> Restarting NV service..."
systemctl --user restart nv.service

echo "==> Waiting for service to start..."
sleep 3

# Verify
ACTIVE=$(systemctl --user is-active nv.service 2>/dev/null || true)
if [ "$ACTIVE" = "active" ]; then
    echo "    systemd: active"
else
    echo "    systemd: $ACTIVE (expected 'active')"
    echo "    Check logs: journalctl --user -u nv -n 50"
    exit 1
fi

if curl -sf "http://127.0.0.1:${HEALTH_PORT}/health" > /dev/null 2>&1; then
    echo "    health endpoint: ok"
else
    echo "    health endpoint: not responding (may still be initializing)"
fi

echo ""
echo "NV installed successfully."
echo "  Binaries:  $INSTALL_DIR/nv-daemon, $INSTALL_DIR/nv"
echo "  Config:    $NV_DIR/nv.toml"
echo "  Service:   $SERVICE_DIR/nv.service"
echo "  Logs:      journalctl --user -u nv -f"
echo "  Status:    nv status"
