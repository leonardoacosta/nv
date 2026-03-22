#!/usr/bin/env python3
"""Discord → Nova relay bot.

Listens for DMs and mentions in configured channels, forwards to Nova's
Telegram bot as [Discord: #channel — @user] prefixed messages.

Config via environment variables:
  DISCORD_BOT_TOKEN    — Discord bot token
  TELEGRAM_BOT_TOKEN   — Nova's Telegram bot token
  TELEGRAM_CHAT_ID     — Leo's Telegram chat ID
  DISCORD_CHANNELS     — Comma-separated channel IDs to watch (optional, watches all if empty)
"""

import os
import asyncio
import aiohttp
import discord

TELEGRAM_API = "https://api.telegram.org/bot{token}/sendMessage"

# Config
DISCORD_TOKEN = os.environ["DISCORD_BOT_TOKEN"]
TG_TOKEN = os.environ["TELEGRAM_BOT_TOKEN"]
TG_CHAT_ID = int(os.environ["TELEGRAM_CHAT_ID"])
WATCH_CHANNELS = set(
    int(c.strip())
    for c in os.environ.get("DISCORD_CHANNELS", "").split(",")
    if c.strip()
)

intents = discord.Intents.default()
intents.message_content = True
intents.dm_messages = True
client = discord.Client(intents=intents)


async def forward_to_telegram(text: str) -> None:
    """Send a message to Nova via Telegram Bot API."""
    url = TELEGRAM_API.format(token=TG_TOKEN)
    async with aiohttp.ClientSession() as session:
        await session.post(url, json={
            "chat_id": TG_CHAT_ID,
            "text": text,
            "parse_mode": "Markdown",
        })


@client.event
async def on_ready():
    print(f"Discord relay connected as {client.user}")
    if WATCH_CHANNELS:
        print(f"Watching channels: {WATCH_CHANNELS}")
    else:
        print("Watching all channels (no filter)")


@client.event
async def on_message(message: discord.Message):
    # Skip own messages
    if message.author == client.user:
        return

    # DMs — always forward
    if isinstance(message.channel, discord.DMChannel):
        text = f"[Discord DM — @{message.author.name}]\n{message.content}"
        await forward_to_telegram(text)
        return

    # Channel messages — check if we're watching this channel
    if WATCH_CHANNELS and message.channel.id not in WATCH_CHANNELS:
        return

    # Check if bot is mentioned or message is in a watched channel
    is_mentioned = client.user in message.mentions
    is_watched = not WATCH_CHANNELS or message.channel.id in WATCH_CHANNELS

    if is_mentioned or is_watched:
        channel_name = getattr(message.channel, "name", "unknown")
        guild_name = getattr(message.guild, "name", "DM")
        text = (
            f"[Discord: {guild_name}/#{channel_name} — @{message.author.name}]\n"
            f"{message.content}"
        )
        await forward_to_telegram(text)


if __name__ == "__main__":
    client.run(DISCORD_TOKEN)
