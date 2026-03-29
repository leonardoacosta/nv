# Implementation Tasks
<!-- beads:epic:nv-u75v -->

## DB Batch

(No DB tasks)

## API Batch

- [x] [2.1] [P-1] Create `packages/daemon/src/telegram/callback-router.ts` — export `CallbackMeta` interface (`callbackQueryId: string`, `messageId: number`, `chatId: string`), `CallbackHandler` type (`(id: string, meta: CallbackMeta, msg: Message) => void`), and `CallbackRouter` class with `register(prefix: string, handler: CallbackHandler): void` and `route(msg: Message): boolean`; `route` MUST extract `callbackQueryId` + `originalMessageId` from `msg.metadata` once, then iterate registered prefixes in insertion order, invoke first match with `(data.slice(prefix.length), meta, msg)`, and return `true`; return `false` if no prefix matches [owner:api-engineer]
- [x] [2.2] [P-1] Refactor `packages/daemon/src/index.ts` — construct a `CallbackRouter` instance before the `telegram.onMessage(...)` call; register all 9 handlers (digest log-and-return, `reminder:done:`, `reminder:snooze:` with nested `lastIndexOf` parse, `watcher:` passing full `msg.text ?? ""` to `handleWatcherCallback`, `OBLIGATION_CONFIRM_PREFIX`, `OBLIGATION_REOPEN_PREFIX`, `OBLIGATION_ESCALATION_RETRY_PREFIX`, `OBLIGATION_ESCALATION_DISMISS_PREFIX`, `OBLIGATION_ESCALATION_TAKEOVER_PREFIX`); replace the 8-branch `if/startsWith` block with `if (callbackRouter.route(msg)) return;` at the same position in the handler body [owner:api-engineer]
- [x] [2.3] [P-1] Run `pnpm tsc --noEmit` in `packages/daemon/` and confirm zero type errors after both files are written [owner:api-engineer]

## UI Batch

(No UI tasks)

## E2E Batch

- [x] [4.1] Verify structural correctness — confirm `data.startsWith("digest:")`, `data.startsWith("reminder:")`, `data.startsWith("watcher:")`, `data.startsWith(OBLIGATION_CONFIRM_PREFIX)` no longer appear in `packages/daemon/src/index.ts`; confirm `callbackQueryId` metadata cast appears exactly once (inside `callback-router.ts`) and zero times in `index.ts` [owner:e2e-engineer]
