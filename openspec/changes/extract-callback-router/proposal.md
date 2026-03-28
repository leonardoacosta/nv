# Proposal: Extract Callback Router

## Change ID
`extract-callback-router`

## Summary
Extract the 8-branch inline `if (data.startsWith(...))` callback dispatch from `packages/daemon/src/index.ts` into a `CallbackRouter` class with a `Map<string, CallbackHandler>` that matches prefixes and extracts shared metadata (`callbackQueryId`, `originalMessageId`). Register all handlers declaratively at startup. Shrinks `index.ts` by ~120 lines.

## Context
- Depends on: none
- Conflicts with: none
- Roadmap: standalone refactor, no wave dependency
- Target file: `packages/daemon/src/index.ts` lines ~372–490 (the `telegram.onMessage()` handler block)
- New file: `packages/daemon/src/telegram/callback-router.ts`

## Motivation
The `telegram.onMessage()` handler in `index.ts` contains 8 sequential `if (data.startsWith(...))` branches for callback routing — digest, reminder (done + snooze), watcher, obligation confirm/reopen, and escalation retry/dismiss/takeover. Each branch (except digest) duplicates the same metadata extraction:

```typescript
const callbackQueryId = String(
  (msg.metadata as { callbackQueryId?: string } | undefined)
    ?.callbackQueryId ?? "",
);
const messageId = Number(
  (msg.metadata as { originalMessageId?: number } | undefined)
    ?.originalMessageId ?? 0,
);
```

This boilerplate appears 7 times across 120 lines. Problems:

1. **Boilerplate repetition**: Every handler independently casts and extracts `callbackQueryId` and `originalMessageId`. Adding a new callback type requires copy-pasting this block.
2. **Unscalable dispatch**: A sequential chain of `if/startsWith` must be scanned linearly. As callback prefixes grow, the dispatch path degrades and the condition order becomes load-bearing.
3. **Coupling to index.ts**: All callback logic is inlined in the daemon entry point. The file cannot be read without mentally parsing through routing logic to find daemon startup code.
4. **No single registration point**: There is no place to see all registered callback prefixes at a glance — they are spread across 8 `if` branches.

A `CallbackRouter` class accepts a `Map<prefix, handler>` registration at construction time, extracts metadata once before dispatch, and exposes a single `route(msg)` call. `index.ts` is reduced to handler registration declarations and a single `router.route(msg)` call.

## Requirements

### Req-1: Create `CallbackRouter` class
Add `packages/daemon/src/telegram/callback-router.ts` exporting a `CallbackRouter` class. The class MUST accept handler registrations via `register(prefix: string, handler: CallbackHandler)`, route an incoming message via `route(msg: Message): boolean` (returns `true` if a handler was matched and invoked, `false` otherwise), and extract `callbackQueryId` and `originalMessageId` from `msg.metadata` exactly once before dispatch. The `CallbackHandler` type SHALL be:

```typescript
export type CallbackHandler = (
  id: string,
  meta: CallbackMeta,
  msg: Message,
) => void;

export interface CallbackMeta {
  callbackQueryId: string;
  messageId: number;
  chatId: string;
}
```

`id` is the portion of `msg.text` after the matched prefix. The router MUST iterate registered prefixes in insertion order and invoke the first matching handler.

### Req-2: Extract metadata once per dispatch
`route(msg)` MUST extract `callbackQueryId` and `originalMessageId` from `msg.metadata` a single time, before testing any prefix. Every handler invocation MUST receive pre-extracted `CallbackMeta` — no handler SHALL re-cast `msg.metadata` internally.

### Req-3: Register all 8 callback handlers in `index.ts`
The `telegram.onMessage()` handler in `index.ts` MUST replace all `if (data.startsWith(...))` callback branches with a `CallbackRouter` instance constructed before the `onMessage` call. The following prefixes MUST be registered:

| Prefix | Handler |
|--------|---------|
| `"digest:"` | Log and return (acknowledge only) |
| `"reminder:done:"` | `handleReminderDone` |
| `"reminder:snooze:"` | `handleReminderSnooze` (parse `duration` and `reminderId` from `id`) |
| `"watcher:"` | `handleWatcherCallback` (pass full `data` not just `id`) |
| `OBLIGATION_CONFIRM_PREFIX` | `handleObligationConfirm` |
| `OBLIGATION_REOPEN_PREFIX` | `handleObligationReopen` |
| `OBLIGATION_ESCALATION_RETRY_PREFIX` | `handleEscalationRetry` |
| `OBLIGATION_ESCALATION_DISMISS_PREFIX` | `handleEscalationDismiss` |
| `OBLIGATION_ESCALATION_TAKEOVER_PREFIX` | `handleEscalationTakeover` |

The `reminder:snooze:` handler MUST preserve the existing parse logic: `rest = id` (the portion after `"reminder:snooze:"`), `lastColon = rest.lastIndexOf(":")`, `duration = rest.slice(0, lastColon)`, `reminderId = rest.slice(lastColon + 1)`.

The `watcher:` handler is a special case: `handleWatcherCallback` receives the full `data` string (not just the portion after the prefix) as its first argument. The registered handler MUST pass `msg.text ?? ""` rather than `id`.

### Req-4: Replace dispatch block with a single `router.route()` call
After registering all handlers, the `telegram.onMessage()` handler MUST replace the 8-branch `if/startsWith` block with:

```typescript
if (callbackRouter.route(msg)) return;
```

This MUST appear at the top of the handler, before the `obligationExecutor.notifyActivity()` call, at the same position the current callback blocks occupy.

### Req-5: `digest:` special case — no metadata needed
The `digest:` handler only logs and returns. Its registration MUST use the `CallbackHandler` signature for consistency, but the implementation MAY ignore `meta`. The existing log call (`log.info(...)`) MUST be preserved.

### Req-6: TypeScript must pass after refactor
Running `pnpm tsc --noEmit` in `packages/daemon/` MUST produce zero errors after all changes are complete.

## Scope
- **IN**: Create `packages/daemon/src/telegram/callback-router.ts`, refactor the `telegram.onMessage()` callback dispatch block in `packages/daemon/src/index.ts`
- **OUT**: Changes to any individual callback handler implementation (`handleReminderDone`, `handleWatcherCallback`, obligation handlers, etc.), changes to `TelegramAdapter`, changes to any file outside `packages/daemon/src/`

## Impact
| File | Change |
|------|--------|
| `packages/daemon/src/telegram/callback-router.ts` | NEW — exports `CallbackRouter`, `CallbackHandler`, `CallbackMeta` |
| `packages/daemon/src/index.ts` | Remove ~120 lines of `if/startsWith` dispatch; add `CallbackRouter` construction and `router.route(msg)` call |

## Risks
| Risk | Mitigation |
|------|-----------|
| `watcher:` prefix requires full `data` not just suffix | `watcher:` handler registered with a wrapper that ignores `id` and passes `msg.text ?? ""` directly to `handleWatcherCallback` — requirement makes this explicit |
| `reminder:snooze:` has a nested parse step | The parse logic (`lastIndexOf(":")`) is moved into the handler closure registered for `"reminder:snooze:"` — it is not lost, just relocated |
| Prefix collision if two prefixes share a stem | Insertion order is preserved (Map iterates insertion order in JS); register more-specific prefixes (`"reminder:done:"`, `"reminder:snooze:"`) before their common stem if any exist. Currently no collisions exist — `"digest:"`, `"reminder:done:"`, `"reminder:snooze:"`, `"watcher:"` are all disjoint |
| Obligation prefixes are string constants, not literals | Constants (`OBLIGATION_CONFIRM_PREFIX`, etc.) are passed directly to `register()` — the router does a `data.startsWith(prefix)` check, so runtime values work identically to string literals |
