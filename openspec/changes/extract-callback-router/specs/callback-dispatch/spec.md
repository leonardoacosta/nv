# Callback Dispatch Router

## ADDED Requirements

### Requirement: CallbackRouter class with prefix-based dispatch

The `packages/daemon/src/telegram/callback-router.ts` module MUST export a `CallbackRouter` class that accepts handler registrations via `register(prefix: string, handler: CallbackHandler)` and routes incoming messages via `route(msg: Message): boolean`. The `route` method MUST iterate registered prefixes in insertion order and invoke the first matching handler, returning `true` on match and `false` otherwise. The `CallbackHandler` type SHALL accept `(id: string, meta: CallbackMeta, msg: Message) => void` where `id` is the portion of `msg.text` after the matched prefix.

#### Scenario: Matching handler is invoked

Given a `CallbackRouter` with a handler registered for `"reminder:done:"`,
when `route()` is called with a message whose text is `"reminder:done:abc123"`,
then the handler is invoked with `id = "abc123"` and `route()` returns `true`.

#### Scenario: No matching handler

Given a `CallbackRouter` with no handler registered for `"unknown:"`,
when `route()` is called with a message whose text is `"unknown:foo"`,
then no handler is invoked and `route()` returns `false`.

### Requirement: Metadata extracted once before dispatch

The `route()` method MUST extract `callbackQueryId` and `originalMessageId` from `msg.metadata` exactly once before testing any prefix. Every handler invocation MUST receive the pre-extracted `CallbackMeta` — no registered handler SHALL re-cast `msg.metadata` internally.

#### Scenario: Metadata passed to handler without re-casting

Given a message with `metadata.callbackQueryId = "q1"` and `metadata.originalMessageId = 42`,
when `route()` dispatches to a registered handler,
then the handler receives `meta.callbackQueryId === "q1"` and `meta.messageId === 42` without performing its own metadata cast.

### Requirement: All 8 callback prefixes registered in index.ts

The `telegram.onMessage()` handler in `packages/daemon/src/index.ts` MUST replace all `if (data.startsWith(...))` callback branches with a `CallbackRouter` instance. All 8 prefixes (`"digest:"`, `"reminder:done:"`, `"reminder:snooze:"`, `"watcher:"`, and the four obligation constants) MUST be registered before the `onMessage` call. The handler body MUST replace the branch block with `if (callbackRouter.route(msg)) return;`. After refactoring, `pnpm tsc --noEmit` in `packages/daemon/` SHALL produce zero errors.

#### Scenario: Single router.route() call replaces 8-branch chain

Given a daemon startup with all 8 handlers registered on the `CallbackRouter`,
when an obligation confirm callback message arrives,
then `callbackRouter.route(msg)` dispatches to `handleObligationConfirm` and returns `true`, and the subsequent `obligationExecutor.notifyActivity()` call is skipped.
