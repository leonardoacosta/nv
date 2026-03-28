# Implementation Tasks

## DB Batch: (none)

No schema changes required.

## API Batch: tRPC Cursor Procedure

- [ ] [1.1] [P-1] Add `chatHistory` procedure to `packages/api/src/routers/message.ts` -- cursor-based query with input `{ limit: z.number().int().min(1).max(50).default(25), cursor: z.string().datetime().optional() }`; when cursor is provided, query `WHERE created_at < cursor ORDER BY created_at DESC LIMIT limit + 1`; when no cursor, `ORDER BY created_at DESC LIMIT limit + 1`; filter to `metadata->>'type' = 'conversation'` OR `metadata->>'type' IS NULL` (exclude tool-call and system messages); if `rows.length > limit`, pop the extra row and set `nextCursor` to oldest row's `createdAt` ISO string; return `{ messages: StoredMessage[], nextCursor: string | null }` [owner:api-engineer]

## UI Batch: Chat Page Rewrite

- [ ] [2.1] [P-1] Replace scroll container in `apps/dashboard/app/chat/page.tsx` -- change the outer `<div ref={scrollRef}>` from `flex-1 overflow-y-auto` to `flex-1 overflow-y-auto flex flex-col-reverse`; add `overflow-anchor: auto` style; remove `bottomRef` ref and its `<div ref={bottomRef} />` scroll anchor element [owner:ui-engineer]
- [ ] [2.2] [P-1] Replace `loadHistory` + `apiFetch` with `useInfiniteQuery` -- import `trpc` from `@/lib/trpc` and `useInfiniteQuery` from `@tanstack/react-query`; call `useInfiniteQuery(trpc.message.chatHistory.infiniteQueryOptions({ limit: 25 }, { getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined }))`; destructure `data`, `fetchNextPage`, `hasNextPage`, `isFetchingNextPage`, `isLoading`, `error`, `refetch`; remove the `loading` state variable, `loadHistory` useCallback, and its mounting `useEffect`; remove the `.reverse()` call on line 348 [owner:ui-engineer]
- [ ] [2.3] [P-1] Add `pendingMessages` state -- `useState<StoredMessage[]>([])` for optimistically-added user messages and completed Nova responses; compute `allMessages` as `[...pendingMessages, ...(data?.pages.flatMap(page => page.messages) ?? [])]`; in `handleSend`, append user message to `pendingMessages` instead of `messages`; on SSE `done` event, append final Nova message to `pendingMessages` and clear streaming state [owner:ui-engineer]
- [ ] [2.4] [P-1] Add `IntersectionObserver` for upward pagination -- create `sentinelRef` on a `<div>` placed above the message list (sibling inside the `column-reverse` container); create observer with `{ root: scrollRef.current, rootMargin: "200px 0px 0px 0px" }`; on intersection, call `fetchNextPage()` when `hasNextPage && !isFetchingNextPage`; show a small spinner while `isFetchingNextPage` is true; disconnect observer on cleanup [owner:ui-engineer]
- [ ] [2.5] [P-2] Update `scrollToBottom` to use `column-reverse` semantics -- change from `bottomRef.current?.scrollIntoView({ behavior: "smooth" })` to `scrollRef.current.scrollTop = 0`; only call after sending a message (not on every `messages` change); remove the `useEffect` that called `scrollToBottom` on `[messages, streamingText]` [owner:ui-engineer]
- [ ] [2.6] [P-2] Wire error state to `useInfiniteQuery` error -- replace the manual `error` state for history loading with the query's `error` object; keep the manual `error` state for send failures only; show `ErrorBanner` with `error.message` and `onRetry={() => void refetch()}` [owner:ui-engineer]
- [ ] [2.7] [P-2] Add query invalidation after message send -- after SSE `done` event and Nova message is appended to `pendingMessages`, call `queryClient.invalidateQueries({ queryKey: trpc.message.chatHistory.queryKey() })` to sync server state; on refetch completion, diff `pendingMessages` against the first page's messages and remove any that are now in the cache [owner:ui-engineer]
- [ ] [2.8] [P-3] Delete `apps/dashboard/app/api/chat/history/route.ts` -- verify no other file imports or fetches from `/api/chat/history`; remove the file [owner:ui-engineer]

## E2E Batch: Verification

- [ ] [3.1] TypeScript compilation: `pnpm --filter @nova/api typecheck` passes with no errors [owner:api-engineer]
- [ ] [3.2] TypeScript compilation: `pnpm --filter nova-dashboard typecheck` passes with no errors [owner:ui-engineer]
- [ ] [3.3] Dashboard build: `pnpm --filter nova-dashboard build` completes successfully [owner:ui-engineer]
- [ ] [3.4] [user] Manual smoke: navigate to `/chat`, verify initial 25 messages load with newest at bottom, no `.reverse()` flicker, scroll position starts at bottom
- [ ] [3.5] [user] Manual smoke: scroll up to top of loaded messages, verify older messages load automatically via IntersectionObserver with a spinner indicator
- [ ] [3.6] [user] Manual smoke: scroll up through multiple pages, verify no scroll position jump when new pages load
- [ ] [3.7] [user] Manual smoke: send a message, verify user bubble appears at bottom, SSE streaming works, Nova response renders correctly
- [ ] [3.8] [user] Manual smoke: after loading multiple pages of history, send a message and verify the view scrolls to bottom correctly
- [ ] [3.9] [user] Manual smoke: verify `/api/chat/history` route returns 404 (deleted)
