#!/usr/bin/env bash
set -euo pipefail

# ── Validate environment ────────────────────────────────────────────────────
if [[ -z "${TELEGRAM_BOT_TOKEN:-}" || -z "${TELEGRAM_CHAT_ID:-}" ]]; then
  echo "Usage: TELEGRAM_BOT_TOKEN=<token> TELEGRAM_CHAT_ID=<chat_id> $0" >&2
  echo "Both TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID must be set." >&2
  exit 1
fi

API_BASE="https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}"
TIMEOUT=30
POLL_INTERVAL=2

# ── Send ping ───────────────────────────────────────────────────────────────
echo "Sending ping..."
SEND_RESPONSE=$(curl -s -X POST "${API_BASE}/sendMessage" \
  -H "Content-Type: application/json" \
  -d "{\"chat_id\": \"${TELEGRAM_CHAT_ID}\", \"text\": \"ping\"}")

if [[ $(echo "${SEND_RESPONSE}" | jq -r '.ok') != "true" ]]; then
  echo "Failed to send ping message." >&2
  echo "Response: ${SEND_RESPONSE}" >&2
  exit 1
fi

PING_MSG_ID=$(echo "${SEND_RESPONSE}" | jq -r '.result.message_id')
echo "Ping sent (message_id=${PING_MSG_ID}). Waiting for pong..."

START_TIME=$(date +%s)

# Use an offset just past the ping message so we only see new updates.
# Start with offset=-1 to fetch the latest update id, then advance from there.
OFFSET=0

# Seed offset to avoid replaying old messages
INIT_UPDATES=$(curl -s "${API_BASE}/getUpdates?offset=-1&limit=1")
LAST_UPDATE_ID=$(echo "${INIT_UPDATES}" | jq -r '.result[-1].update_id // empty')
if [[ -n "${LAST_UPDATE_ID}" ]]; then
  OFFSET=$(( LAST_UPDATE_ID + 1 ))
fi

# ── Poll for pong ───────────────────────────────────────────────────────────
while true; do
  NOW=$(date +%s)
  ELAPSED=$(( NOW - START_TIME ))

  if (( ELAPSED >= TIMEOUT )); then
    echo "Timeout: no pong received within ${TIMEOUT} seconds." >&2
    exit 1
  fi

  UPDATES=$(curl -s "${API_BASE}/getUpdates?offset=${OFFSET}&limit=10&timeout=5")

  UPDATE_COUNT=$(echo "${UPDATES}" | jq '.result | length')

  if (( UPDATE_COUNT > 0 )); then
    # Advance offset past all retrieved updates
    LAST_ID=$(echo "${UPDATES}" | jq -r '.result[-1].update_id')
    OFFSET=$(( LAST_ID + 1 ))

    # Check each update for a pong reply to our ping
    FOUND=$(echo "${UPDATES}" | jq --argjson ping_id "${PING_MSG_ID}" '
      .result[] |
      select(
        .message.text == "pong" and
        .message.reply_to_message.message_id == $ping_id
      ) |
      .message.message_id
    ' | head -1)

    if [[ -n "${FOUND}" ]]; then
      END_TIME=$(date +%s)
      ELAPSED_FINAL=$(( END_TIME - START_TIME ))
      echo "pong received in ${ELAPSED_FINAL}s"
      exit 0
    fi
  fi

  sleep "${POLL_INTERVAL}"
done
