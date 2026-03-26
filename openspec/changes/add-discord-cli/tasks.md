# Implementation Tasks

<!-- beads:epic:nv-mh1o -->

## API Batch

- [ ] [1.1] [P-1] Create `packages/tools/discord-cli/` directory with `package.json` (commander + esbuild dev dep, build + install-cli scripts, shebang prepend) and `tsconfig.json` (Node 20, strict, moduleResolution bundler) [owner:api-engineer]
- [ ] [1.2] [P-1] Create `src/auth.ts` — `DiscordClient` class: reads `DISCORD_BOT_TOKEN`, exits 1 with error if missing, attaches `Authorization: Bot {token}` header, exposes `get(path): Promise<unknown>` via built-in `fetch`, handles HTTP 429 with one Retry-After retry [owner:api-engineer]
- [ ] [1.3] [P-1] Create `src/commands/guilds.ts` — `guildsCommand(client)`: calls `GET /users/@me/guilds`, formats as `Guilds (N)\n{id}  {name}` per line; prints "Bot is not a member of any guilds." on empty [owner:api-engineer]
- [ ] [1.4] [P-1] Create `src/commands/channels.ts` — `channelsCommand(client, guildId)`: calls `GET /guilds/{guildId}/channels`, builds category map from type-4 channels, filters to type-0 text channels, sorts by position, renders grouped output; handles 404 as "Guild not found: {id}" exit 1 [owner:api-engineer]
- [ ] [1.5] [P-1] Create `src/commands/messages.ts` — `messagesCommand(client, channelId, limit)`: calls `GET /channels/{channelId}/messages?limit={limit}`, filters to type-0 messages, formats `[{relative}] {author}: {content}` with 500-char truncation; handles 403/404 with specific exit-1 messages [owner:api-engineer]
- [ ] [1.6] [P-1] Create `src/index.ts` — commander program with three subcommands (`guilds`, `channels <guild-id>`, `messages <channel-id> [--limit N]`); constructs `DiscordClient`, routes to command functions, propagates non-zero exits [owner:api-engineer]
- [ ] [1.7] [P-2] Add `relativeTime(iso: string): string` helper (shared utility, inline in `src/utils.ts`) — same bucketing as `nv-tools::relative_time`: "just now", "Nm ago", "Nh ago", "Nd ago", "Mon D" [owner:api-engineer]
- [ ] [1.8] [P-2] Verify `npm run build` produces `dist/discord-cli.js` and `npm run install-cli` installs to `~/.local/bin/discord-cli` with correct shebang and execute bit [owner:api-engineer]

## Verify

- [ ] [2.1] `npm run build` exits 0, `dist/discord-cli.js` exists [owner:api-engineer]
- [ ] [2.2] Unit test: `relativeTime` covers all time buckets (just now, minutes, hours, days, older) [owner:api-engineer]
- [ ] [2.3] Unit test: channels command groups text channels by category and sorts by position [owner:api-engineer]
- [ ] [2.4] Unit test: messages command truncates content over 500 chars [owner:api-engineer]
- [ ] [2.5] [user] Manual: `discord-cli guilds` — verify bot guild list renders [owner:api-engineer]
- [ ] [2.6] [user] Manual: `discord-cli channels <guild-id>` — verify channel grouping output [owner:api-engineer]
- [ ] [2.7] [user] Manual: `discord-cli messages <channel-id> --limit 10` — verify message list renders [owner:api-engineer]
