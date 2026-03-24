# Channels — Audit Memory

## Route Inventory
- POST /webhooks/teams (MS Graph subscriptions)
- POST /webhooks/jira (conditional, Jira events)

## Key Modules
- channels/discord/ — DiscordRestClient, bot token auth, 2000 char chunking
- channels/telegram/ — TelegramClient, long polling, inline keyboards
- channels/teams/ — TeamsClient, MS Graph OAuth, 60min subscription lifecycle
- channels/email/ — EmailClient, MS Graph OAuth, MIME parsing
- channels/imessage/ — BlueBubblesClient, timestamp polling
- messages.rs — MessageStore (SQLite), StoredMessage, StatsReport

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
