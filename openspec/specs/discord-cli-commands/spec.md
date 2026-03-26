# discord-cli-commands Specification

## Purpose
TBD - created by archiving change add-discord-cli. Update Purpose after archive.
## Requirements
### Requirement: DiscordClient — bot token auth

A `DiscordClient` class in `src/auth.ts` SHALL read `DISCORD_BOT_TOKEN` from the environment and attach `Authorization: Bot {token}` to every outbound request. It MUST expose a `get(path: string): Promise<unknown>` method using Node built-in `fetch`. It MUST fail fast on a missing env var (exit 1, message to stderr). It MUST handle HTTP 429 by honoring the `Retry-After` header and retrying once.

#### Scenario: successful GET
Given `DISCORD_BOT_TOKEN` is set
When `client.get('/users/@me/guilds')` is called
Then the response body is returned as parsed JSON.

#### Scenario: missing token
Given `DISCORD_BOT_TOKEN` is absent
When `DiscordClient` is constructed
Then an error message is printed to stderr and the process exits 1.

#### Scenario: 429 rate limit
Given the Discord API returns 429 with `Retry-After: 2`
When `client.get(...)` encounters the 429
Then the client waits 2 seconds and retries once before surfacing an error.

---

### Requirement: guilds subcommand

The `guilds` command SHALL call `GET /users/@me/guilds` and print all guilds the bot is a member of. Output MUST begin with a `Guilds (N)` count header followed by one line per guild: `{id}  {name}`.

#### Scenario: guilds present
Given the bot is in 3 guilds
When `discord-cli guilds` is run
Then output begins with `Guilds (3)` and lists each guild's ID and name.

#### Scenario: empty guild list
Given the bot is in no guilds
When `discord-cli guilds` is run
Then output is `Bot is not a member of any guilds.` and exit code is 0.

---

### Requirement: channels subcommand

The `channels <guild-id>` command SHALL call `GET /guilds/{guild_id}/channels`, filter to text channels (type 0), group them under their parent category (type 4), and sort by position within each group. Channels with no parent MUST appear under `(uncategorized)`.

#### Scenario: channels grouped by category
Given a guild with categories "General" and "Dev" each containing text channels
When `discord-cli channels {id}` is run
Then channels appear indented beneath their category headers, sorted by position.

#### Scenario: guild not found (404)
Given Discord returns 404 for the guild ID
When `discord-cli channels {id}` is run
Then `Guild not found: {id}` is printed to stderr and exit code is 1.

#### Scenario: no text channels
Given the guild has no type-0 channels
When `discord-cli channels {id}` is run
Then `No text channels found in guild {id}.` is printed and exit code is 0.

---

### Requirement: messages subcommand

The `messages <channel-id> [--limit N]` command SHALL call `GET /channels/{channel_id}/messages?limit={limit}` (default 50, max 100), skip non-user messages (type !== 0), format each message as `[{relative}] {author}: {content}`, and truncate content exceeding 500 characters with a `…` suffix.

#### Scenario: messages present
Given the channel has 10 recent messages
When `discord-cli messages {id} --limit 10` is run
Then output shows a `Messages — #<name> (last 10)` header and one formatted line per message with relative timestamps.

#### Scenario: channel not found (404)
Given Discord returns 404 for the channel ID
When `discord-cli messages {id}` is run
Then `Channel not found: {id}` is printed to stderr and exit code is 1.

#### Scenario: no read permission (403)
Given Discord returns 403 for the channel ID
When `discord-cli messages {id}` is run
Then `No permission to read channel {id}` is printed to stderr and exit code is 1.

#### Scenario: content over 500 chars
Given a message with content longer than 500 characters
When it is formatted
Then the content is truncated to 500 characters and `…` is appended.

---

### Requirement: build and install

`npm run build` SHALL produce `dist/discord-cli.js` as a single esbuild bundle with a `#!/usr/bin/env node` shebang. `npm run install-cli` SHALL copy the bundle to `~/.local/bin/discord-cli` and set the execute bit.

#### Scenario: build and install
Given the package dependencies are installed
When `npm run build && npm run install-cli` is run
Then `~/.local/bin/discord-cli` exists with execute permission and `discord-cli --help` prints the command list.

