#!/usr/bin/env python3
"""Teams → Nova webhook relay.

Receives HTTP POST from Power Automate, forwards to Nova's Telegram bot.
Power Automate triggers on Teams channel messages/DMs, sends JSON payload here.

Config via environment variables:
  TELEGRAM_BOT_TOKEN   — Nova's Telegram bot token
  TELEGRAM_CHAT_ID     — Leo's Telegram chat ID
  TEAMS_WEBHOOK_PORT   — Port to listen on (default: 8401)
  TEAMS_WEBHOOK_SECRET — Shared secret for request validation (optional)
"""

import os
import json
import asyncio
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request

TELEGRAM_API = "https://api.telegram.org/bot{token}/sendMessage"

TG_TOKEN = os.environ["TELEGRAM_BOT_TOKEN"]
TG_CHAT_ID = int(os.environ["TELEGRAM_CHAT_ID"])
PORT = int(os.environ.get("TEAMS_WEBHOOK_PORT", "8401"))
SECRET = os.environ.get("TEAMS_WEBHOOK_SECRET", "")


def forward_to_telegram(text: str) -> None:
    """Send a message to Nova via Telegram Bot API (sync)."""
    url = TELEGRAM_API.format(token=TG_TOKEN)
    data = json.dumps({
        "chat_id": TG_CHAT_ID,
        "text": text,
        "parse_mode": "Markdown",
    }).encode()
    req = urllib.request.Request(
        url, data=data, headers={"Content-Type": "application/json"}
    )
    urllib.request.urlopen(req, timeout=10)


class WebhookHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        # Optional secret validation
        if SECRET:
            auth = self.headers.get("X-Webhook-Secret", "")
            if auth != SECRET:
                self.send_response(401)
                self.end_headers()
                self.wfile.write(b"Unauthorized")
                return

        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length)

        try:
            payload = json.loads(body)
        except json.JSONDecodeError:
            self.send_response(400)
            self.end_headers()
            self.wfile.write(b"Invalid JSON")
            return

        # Power Automate payload format (customize to match your flow)
        sender = payload.get("from", payload.get("sender", "Unknown"))
        channel = payload.get("channel", payload.get("channelName", "Unknown"))
        team = payload.get("team", payload.get("teamName", ""))
        content = payload.get("body", payload.get("content", payload.get("message", "")))

        # Strip HTML tags from Teams messages (they come as HTML)
        import re
        content = re.sub(r"<[^>]+>", "", content).strip()

        location = f"{team}/#{channel}" if team else channel
        text = f"[Teams: {location} — {sender}]\n{content}"

        try:
            forward_to_telegram(text)
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"OK")
        except Exception as e:
            print(f"Error forwarding to Telegram: {e}")
            self.send_response(502)
            self.end_headers()
            self.wfile.write(b"Telegram forward failed")

    def do_GET(self):
        """Health check."""
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'{"status":"ok"}')

    def log_message(self, format, *args):
        """Compact logging."""
        print(f"[teams-relay] {args[0]}")


if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", PORT), WebhookHandler)
    print(f"Teams webhook relay listening on :{PORT}")
    print(f"Power Automate → POST http://homelab:{PORT}/")
    server.serve_forever()
