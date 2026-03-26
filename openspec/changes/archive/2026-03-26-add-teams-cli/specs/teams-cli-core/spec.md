# Capability: Teams CLI Core

## ADDED Requirements

### Requirement: Package Scaffold
`packages/tools/teams-cli/` SHALL be created as a standalone TypeScript package with `package.json` (scripts: `build` via esbuild, `install-cli` to copy dist to `~/.local/bin/teams-cli`), `tsconfig.json` (strict, Node20 target), and source files under `src/`.

#### Scenario: Package builds cleanly
Given all source files are present
When `pnpm build` is run from `packages/tools/teams-cli/`
Then `dist/teams-cli.js` is produced with a `#!/usr/bin/env node` shebang
And the file is executable after running `install-cli`

#### Scenario: Missing env vars
Given `MS_GRAPH_CLIENT_ID` is not set in the environment
When any subcommand is invoked
Then stderr contains "MS Graph not configured — MS_GRAPH_CLIENT_ID env var not set"
And the process exits with code 1

### Requirement: MsGraphClient Auth
`src/auth.ts` SHALL export a `MsGraphClient` class that reads `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_CLIENT_SECRET`, and `MS_GRAPH_TENANT_ID` from environment, fetches a client_credentials token from `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`, caches the token in memory until expiry, and exposes `get(url)` and `post(url, body)` helpers that inject `Authorization: Bearer {token}`.

#### Scenario: Token fetch and cache
Given valid credentials in environment
When `get()` is called twice within the token lifetime
Then the token endpoint is called only once
And both calls succeed with a valid Bearer token

#### Scenario: Graph API 403
Given credentials without the required permission
When any Graph API call returns 403
Then stderr contains the permission name required (e.g. "Chat.Read.All required")
And the process exits with code 1

### Requirement: chats Command
`src/commands/chats.ts` SHALL implement `teams-cli chats [--limit N]` calling `GET /chats?$top={limit}&$expand=members&$orderby=lastMessageReceivedDateTime desc`, formatting output as one chat per line with topic (DM chats show "DM: {other member name}"), type badge, and relative last-active timestamp.

#### Scenario: List recent chats
Given valid credentials with Chat.Read.All
When `teams-cli chats --limit 5` is run
Then stdout lists up to 5 chats
And DM entries show the other participant's display name prefixed with "DM:"
And each line includes a relative timestamp (e.g. "2h ago")

#### Scenario: Insufficient permissions
Given credentials without Chat.Read.All
When `teams-cli chats` is run
Then stderr contains "Chat.Read.All required"
And the process exits with code 1

### Requirement: read-chat Command
`src/commands/chats.ts` SHALL implement `teams-cli read-chat <id> [--limit N]` calling `GET /chats/{id}/messages?$top={limit}` ordered newest-first, stripping HTML tags from message bodies.

#### Scenario: Read chat messages
Given a valid chat ID and Chat.Read.All permission
When `teams-cli read-chat 19:abc@thread.v2 --limit 10` is run
Then stdout shows up to 10 messages
And each message line is formatted as `[relative_time] Display Name: plain text body`
And HTML tags are absent from message bodies

#### Scenario: Chat not found
Given an invalid or non-existent chat ID
When `teams-cli read-chat invalid-id` is run
Then stderr contains "Not found: invalid-id"
And the process exits with code 1

### Requirement: channels Command
`src/commands/channels.ts` SHALL implement `teams-cli channels <team-id>` calling `GET /teams/{team-id}/channels` and listing each channel with display name and ID.

#### Scenario: List channels
Given a valid team ID and ChannelMessage.Read.All permission
When `teams-cli channels {team-id}` is run
Then stdout lists all channels, one per line, with name and ID

### Requirement: messages Command
`src/commands/messages.ts` SHALL implement `teams-cli messages <team-id> <channel-id> [--limit N]` calling `GET /teams/{team-id}/channels/{channel-id}/messages?$top={limit}` and stripping HTML from message bodies.

#### Scenario: Read channel messages
Given valid team and channel IDs
When `teams-cli messages {team-id} {channel-id} --limit 20` is run
Then stdout shows up to 20 messages with sender, relative timestamp, and plain text body

### Requirement: presence Command
`src/commands/presence.ts` SHALL implement `teams-cli presence <user>` calling `GET /users/{user}/presence` and returning a single-line summary.

#### Scenario: Check user presence
Given a valid UPN and Presence.Read.All permission
When `teams-cli presence sarah@example.com` is run
Then stdout is exactly one line: `sarah@example.com: Available — InACall`

### Requirement: send Command
`src/commands/send.ts` SHALL implement `teams-cli send <chat-id> <message>` posting `{"body":{"content":"{message}"}}` to `POST /chats/{chat-id}/messages` and printing "Sent." on success.

#### Scenario: Send message
Given a valid chat ID and ChatMessage.Send permission
When `teams-cli send 19:abc@thread.v2 "Hello from Nova"` is run
Then the message is delivered to the chat
And stdout contains "Sent."

#### Scenario: Missing message argument
Given the send command is invoked without a message argument
When `teams-cli send {chat-id}` is run
Then stderr contains a usage hint
And the process exits with code 1
