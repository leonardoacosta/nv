# Implementation Tasks
<!-- beads:epic:nv-f4sd -->

## API Batch

- [x] [2.1] [P-1] Create `packages/api/src/lib/people-parser.ts` — move `parsePeopleMemory()` and supporting types/functions from `apps/dashboard/lib/entity-resolution/people-parser.ts` to the API package; export `PersonProfile` interface and `parsePeopleMemory` function [owner:api-engineer]
- [x] [2.2] [P-1] Create `packages/api/src/lib/telegram-metadata.ts` — implement `extractTelegramName(metadata: Record<string, unknown> | null): string | null` that extracts display name from Telegram message metadata JSONB; handle nested `from.first_name`/`from.last_name`/`from.username` and flat shapes; return `"first last"` trimmed, fallback to username, fallback to null [owner:api-engineer]
- [x] [2.3] [P-1] Create `packages/api/src/lib/sender-resolver.ts` — implement `resolveSenders(senders: Array<{ raw: string; channel: string; metadata: unknown }>, db: DbClient): Promise<Map<string, SenderResolution>>` that batches resolution across contacts table (single query with channel_ids JSONB match), Telegram metadata extraction, and memory people profiles (single query + parse); returns `{ displayName, avatarInitial, source }` per sender key [owner:api-engineer]
- [x] [2.4] [P-1] Modify `packages/api/src/routers/message.ts` message.list — after fetching rows, collect unique sender+channel pairs, call `resolveSenders()`, attach `senderResolved` object to each mapped message in the response [owner:api-engineer]

## UI Batch

- [ ] [3.1] [P-1] Update `StoredMessage` type in `apps/dashboard/types/api.ts` — add `senderResolved: { displayName: string; avatarInitial: string; source: "contact" | "telegram-meta" | "memory" | "raw" }` field [owner:ui-engineer]
- [ ] [3.2] [P-1] Remove `useContactResolver` hook from `apps/dashboard/app/messages/page.tsx` — delete the hook (lines 105-138), the `trpcClient.contact.resolve.mutate` call, and the `resolvedName` variable passed to `MessageRowDense`; replace with direct `msg.senderResolved.displayName` usage [owner:ui-engineer]
- [ ] [3.3] [P-1] Add sender avatar initial badge to `MessageRowDense` — render a 20px circular badge showing `senderResolved.avatarInitial` in the channel accent color; for outbound (nova) messages use a fixed brand color with "N"; position before sender name in the dense row [owner:ui-engineer]
- [ ] [3.4] [P-2] Show resolution source in expanded message detail — in the expanded section's Sender field, append a subtle "(via contacts)" / "(via Telegram)" / "(via memory)" label based on `senderResolved.source`; raw source shows no label [owner:ui-engineer]
- [ ] [3.5] [P-2] Update `apps/dashboard/lib/entity-resolution/people-parser.ts` — replace implementation with a re-export from `@nova/api` (or keep as-is if API package is not directly importable from dashboard); ensure no other dashboard code breaks [owner:ui-engineer]

## Verify

- [ ] [4.1] `pnpm typecheck` passes across all packages [owner:api-engineer]
- [ ] [4.2] `pnpm build` passes [owner:api-engineer]
- [ ] [4.3] Manual verification: Messages page shows resolved names for Telegram senders (from metadata), "nova" shows branded N badge, raw fallback shows ID unchanged [owner:ui-engineer]
