# Capability: /diary Telegram Command

## ADDED Requirements

### Requirement: /diary command replies with a compact formatted summary of recent entries
The Telegram command handler for `/diary` SHALL call `getEntriesByDate` for the requested or default date, format up to 10 entries as compact text blocks, and send a single Telegram reply. If today has no entries the command MUST fall back to yesterday. The reply MUST be truncated at 4 000 characters with a trailing "...(truncated)" notice if needed.

#### Scenario: /diary with no argument — entries today

Given 3 diary entries exist for today
When the user sends `/diary`
Then the bot replies with a compact summary of the 3 entries (newest first)
And each entry shows: time (HH:MM), trigger type, channel, tools (comma-joined or "none"), token cost, latency ms

#### Scenario: /diary with no argument — today is empty, fallback to yesterday

Given 0 entries for today but 5 entries for yesterday
When the user sends `/diary`
Then the bot replies with yesterday's 5 entries
And the header indicates the date shown

#### Scenario: /diary with YYYY-MM-DD argument

Given entries on 2026-03-20
When the user sends `/diary 2026-03-20`
Then the bot replies with entries for that date

#### Scenario: Character cap prevents message-too-long errors

Given 20 entries that would exceed 4 000 chars when formatted
When the bot builds the reply
Then the message is truncated at 4 000 characters with a trailing "...(truncated)" notice

#### Scenario: No entries at all

Given no entries for today or yesterday
When the user sends `/diary`
Then the bot replies "No diary entries found."
