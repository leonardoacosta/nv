# Implementation Tasks

<!-- beads:epic:nv-t17u -->

## DB Batch

<!-- no DB changes -->

## API Batch

- [x] [1.1] [P-1] Create `packages/tools/teams-cli/package.json` with build and install-cli scripts using esbuild [owner:api-engineer]
- [x] [1.2] [P-1] Create `packages/tools/teams-cli/tsconfig.json` targeting Node20 with strict mode [owner:api-engineer]
- [x] [1.3] [P-1] Implement `src/auth.ts` — `MsGraphClient` with client_credentials token fetch, in-memory cache, and `get`/`post` helpers [owner:api-engineer]
- [x] [1.4] [P-2] Implement `src/commands/chats.ts` — list chats via `GET /chats` with member expansion, formatted plain text output [owner:api-engineer]
- [x] [1.5] [P-2] Implement `src/commands/channels.ts` — list channels via `GET /teams/{id}/channels` [owner:api-engineer]
- [x] [1.6] [P-2] Implement `src/commands/messages.ts` — read channel messages via `GET /teams/{id}/channels/{id}/messages`, HTML stripped [owner:api-engineer]
- [x] [1.7] [P-2] Implement `src/commands/presence.ts` — check user presence via `GET /users/{user}/presence` [owner:api-engineer]
- [x] [1.8] [P-2] Implement `src/commands/send.ts` — send chat message via `POST /chats/{id}/messages` [owner:api-engineer]
- [x] [1.9] [P-1] Implement `src/index.ts` — commander root wiring all six subcommands with --limit option where applicable [owner:api-engineer]
- [x] [1.10] [P-2] Verify `pnpm build` produces `dist/teams-cli.cjs` with shebang and `install-cli` installs to `~/.local/bin/teams-cli` [owner:api-engineer]

## UI Batch

<!-- no UI changes -->

## E2E Batch

- [SKIPPED] [2.1] Smoke test: `teams-cli presence {known-user}` returns valid output against dev credentials [owner:e2e-engineer]
- [SKIPPED] [2.2] Smoke test: `teams-cli chats --limit 3` returns formatted list or clear permission error [owner:e2e-engineer]
