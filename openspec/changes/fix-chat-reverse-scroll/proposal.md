# Proposal: Fix Chat Reverse Scroll + Cursor Pagination

## Change ID
`fix-chat-reverse-scroll`

## Summary
Replace the chat page's fetch-all-then-reverse pattern with proper reverse-chronological rendering using CSS `flex-direction: column-reverse`, migrate from the raw `/api/chat/history` route to a tRPC `message.chatHistory` infinite query with cursor-based pagination, and preserve real-time WebSocket message appending at the bottom.

## Context
- Extends: `apps/dashboard/app/chat/page.tsx` (chat page), `packages/api/src/routers/message.ts` (tRPC message router), `apps/dashboard/app/api/chat/history/route.ts` (raw API route)
- Depends on: `add-trpc-api` (tRPC infrastructure exists -- `packages/api/`, `DashboardRouter`, `trpc` client), `add-chat-page` (chat page exists with SSE streaming), `add-tanstack-query` (TanStack Query provider exists)
- Related: `unify-conversation-streaming` (WebSocket event system for cross-channel streaming -- this spec preserves the existing SSE streaming path and does not touch WebSocket integration)
- Dashboard: Next.js App Router, tRPC v11 with `queryOptions()`/`mutationOptions()` pattern via TanStack Query
- Current behavior: `/api/chat/history` fetches 50 messages `ORDER BY created_at DESC`, client does `.reverse()` and scrolls to bottom

## Motivation

Three UX problems exist in the current chat page:

1. **Client-side `.reverse()` on every load.** The chat history API returns messages newest-first (`ORDER BY created_at DESC LIMIT 50`). The client reverses the array to display oldest-at-top, newest-at-bottom. This is correct visually, but the approach breaks when pagination is added -- you cannot reverse a page of results and maintain correct scroll position.

2. **Hard-coded 50-message limit with no pagination.** The API returns at most 50 messages. For conversations with hundreds of messages, older history is silently lost. There is no "load more" mechanism. WhatsApp/iMessage-style chat UX expects scroll-to-top to load older messages progressively.

3. **Raw `apiFetch` instead of tRPC.** The chat page uses `apiFetch("/api/chat/history")` with manual JSON parsing and no type safety. The tRPC `message.list` procedure already exists in `packages/api/src/routers/message.ts` but the chat page does not use it. A dedicated `message.chatHistory` cursor-based procedure enables `useInfiniteQuery` for seamless upward pagination.

## Requirements

### Req-1: Add `message.chatHistory` tRPC Procedure

Add a `chatHistory` procedure to the existing `messageRouter` in `packages/api/src/routers/message.ts`. This is a cursor-based query optimized for the chat page's reverse-scroll pattern.

**Input schema:**
```typescript
z.object({
  limit: z.number().int().min(1).max(50).default(25),
  cursor: z.string().datetime().optional(), // ISO timestamp of oldest message in current view
})
```

**Query logic:**
- When `cursor` is provided: `WHERE created_at < cursor ORDER BY created_at DESC LIMIT limit + 1`
- When no cursor (initial load): `ORDER BY created_at DESC LIMIT limit + 1`
- The `+ 1` overfetch determines whether more pages exist: if `rows.length > limit`, there are older messages; pop the extra row and set `nextCursor` to the oldest row's `created_at` ISO string.
- No channel filter -- returns all channels (unified timeline, matching current behavior).
- Filter to `type = 'conversation'` by default (exclude tool-call and system messages from chat view) using the `metadata->>'type'` JSONB field, same pattern as the existing `list` procedure.

**Return shape:**
```typescript
{
  messages: StoredMessage[],  // newest-first (descending), limit items max
  nextCursor: string | null,  // ISO timestamp for next page, null if no more
}
```

The `messages` array is returned in **descending** order (newest first). The client's `column-reverse` CSS container renders them correctly without any `.reverse()` call -- the first item in the array appears at the bottom of the container.

#### Scenario: Initial load returns most recent 25 messages

Given 100 messages exist in the database,
when `chatHistory({ limit: 25 })` is called with no cursor,
then 25 messages are returned in descending `created_at` order,
and `nextCursor` is the `created_at` ISO string of the 25th (oldest) message.

#### Scenario: Cursor fetches next page of older messages

Given `nextCursor` is `"2026-03-27T10:00:00.000Z"`,
when `chatHistory({ limit: 25, cursor: "2026-03-27T10:00:00.000Z" })` is called,
then messages with `created_at < "2026-03-27T10:00:00.000Z"` are returned,
and `nextCursor` is set if more messages exist beyond this page.

#### Scenario: Final page returns null cursor

Given only 10 messages exist with `created_at < cursor`,
when `chatHistory({ limit: 25, cursor })` is called,
then 10 messages are returned and `nextCursor` is `null`.

### Req-2: CSS `column-reverse` Scroll Container

Replace the chat page's scroll container to use `flex-direction: column-reverse`. This CSS property renders flex children in reverse order -- the first DOM element appears at the bottom, and the browser's native scroll position starts at the bottom. This eliminates the need for `scrollIntoView` hacks on initial load and provides correct scroll anchoring when older messages are prepended.

**Current markup (simplified):**
```tsx
<div className="flex-1 overflow-y-auto">         {/* scrollRef */}
  <div className="flex flex-col gap-3 py-4">     {/* message list */}
    {messages.map(msg => <MessageBubble />)}
    <div ref={bottomRef} />                       {/* scroll anchor */}
  </div>
</div>
```

**New markup:**
```tsx
<div ref={scrollRef} className="flex-1 overflow-y-auto flex flex-col-reverse">
  <div className="flex flex-col gap-3 py-4">
    {/* Messages rendered newest-first from the data array.
        column-reverse makes the first item appear at the bottom. */}
    {allMessages.map(msg => <MessageBubble />)}
    {sending && !telegramPolling && <StreamingBubble />}
  </div>

  {/* Sentinel for loading older messages -- appears above all messages */}
  {hasNextPage && (
    <div ref={sentinelRef} className="flex justify-center py-4">
      {isFetchingNextPage ? <Spinner /> : null}
    </div>
  )}
</div>
```

With `column-reverse`, the inner `div` is rendered at the bottom of the scroll container. The sentinel for loading older messages is a sibling that appears above. The scroll position naturally starts at the bottom (showing newest messages) and scrolling up reveals older messages.

**Key behaviors:**
- On initial load, the viewport is at the bottom (newest messages visible) without any `scrollIntoView` call -- `column-reverse` does this natively.
- The `bottomRef` scroll anchor is removed -- no longer needed.
- The `scrollToBottom` callback is simplified: only needed after sending a new message (to counteract any scroll drift), not on every `messages` state change.
- `scrollToBottom` uses `scrollRef.current.scrollTop = 0` (in `column-reverse`, `scrollTop = 0` is the bottom).

#### Scenario: Page opens with scroll at bottom

Given 40 messages are loaded,
when the chat page first renders with `flex-direction: column-reverse`,
then the newest messages are visible in the viewport without any scroll animation,
and the scrollbar is at its starting position (bottom of conversation).

#### Scenario: User scrolls up then receives new message

Given the user has scrolled up to read older messages,
when a new message arrives via SSE streaming and is appended to state,
then the scroll position is preserved (user stays where they scrolled),
and a "scroll to bottom" indicator could appear (future enhancement, out of scope).

### Req-3: Infinite Query with `useInfiniteQuery`

Replace the `useEffect` + `apiFetch` history loading with TanStack Query's `useInfiniteQuery` consuming the `message.chatHistory` tRPC procedure.

**Client setup:**
```typescript
import { trpc } from "@/lib/trpc";
import { useInfiniteQuery } from "@tanstack/react-query";

const {
  data,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
  isLoading,
  error,
  refetch,
} = useInfiniteQuery(
  trpc.message.chatHistory.infiniteQueryOptions(
    { limit: 25 },
    {
      getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    },
  ),
);
```

Note: In TanStack Query's `useInfiniteQuery` with `column-reverse`, "next page" semantically means "older messages" (scrolling upward). The `getNextPageParam` returns the cursor for the next batch of older messages.

**Flattening pages:**
```typescript
const allMessages = data?.pages.flatMap(page => page.messages) ?? [];
```

The flattened array is in descending order (newest first across all pages). With `column-reverse`, the first element renders at the bottom -- exactly correct.

**Remove legacy code:**
- Remove the `loadHistory` callback and its `useEffect`
- Remove the `loading` state variable (replaced by `isLoading` from the query)
- Remove the `apiFetch("/api/chat/history")` call
- Remove the `.reverse()` call on line 348

#### Scenario: Initial load uses tRPC infinite query

Given the chat page mounts,
when `useInfiniteQuery` executes the initial fetch,
then `trpc.message.chatHistory` is called with `{ limit: 25 }` and no cursor,
and the loading skeleton is shown while `isLoading` is true,
and messages render correctly in `column-reverse` once data arrives.

#### Scenario: Error state shows error banner with retry

Given the tRPC call fails (network error, server error),
when the error is captured by `useInfiniteQuery`,
then the `ErrorBanner` is shown with the error message,
and the "Try Again" button calls `refetch()`.

### Req-4: Upward Pagination via Scroll Sentinel

Add an `IntersectionObserver` on a sentinel element placed above the message list (inside the `column-reverse` container) to trigger `fetchNextPage()` when the user scrolls to the top.

**Implementation:**
```typescript
const sentinelRef = useRef<HTMLDivElement>(null);

useEffect(() => {
  const sentinel = sentinelRef.current;
  if (!sentinel || !hasNextPage) return;

  const observer = new IntersectionObserver(
    ([entry]) => {
      if (entry?.isIntersecting && hasNextPage && !isFetchingNextPage) {
        void fetchNextPage();
      }
    },
    { root: scrollRef.current, rootMargin: "200px 0px 0px 0px" },
  );

  observer.observe(sentinel);
  return () => observer.disconnect();
}, [hasNextPage, isFetchingNextPage, fetchNextPage]);
```

The `rootMargin: "200px 0px 0px 0px"` triggers the fetch 200px before the user reaches the top, providing a smooth loading experience.

**Loading indicator:** While `isFetchingNextPage` is true, show a small spinner in the sentinel area. When `hasNextPage` is false, show nothing (or optionally "Beginning of conversation" text).

**Scroll position preservation:** TanStack Query's page-based data model combined with `column-reverse` CSS naturally preserves scroll position when new pages are prepended. The browser's scroll anchoring (`overflow-anchor: auto`) keeps the viewport stable as new DOM nodes are inserted above.

#### Scenario: Scrolling to top loads older messages

Given 25 messages are loaded and `hasNextPage` is true,
when the user scrolls up until the sentinel is within 200px of the viewport,
then `fetchNextPage()` is called,
and a spinner appears at the top,
and older messages appear above the current ones without scroll position jumping.

#### Scenario: All messages loaded shows end state

Given all messages have been fetched and `hasNextPage` is false,
when the user scrolls to the very top,
then no fetch is triggered,
and the sentinel is not rendered.

### Req-5: Preserve Real-Time Message Appending

The existing SSE streaming for new messages (send a message, stream Nova's response) must continue working. New messages are appended to the local state independently of the infinite query's cached pages.

**Approach:** Maintain a separate `pendingMessages` state array for optimistically-added user messages and streaming Nova responses. The rendered message list combines the infinite query pages with pending messages:

```typescript
const [pendingMessages, setPendingMessages] = useState<StoredMessage[]>([]);

// All messages: paginated history (newest-first) + pending (also newest-first)
// pendingMessages are newer than any paginated data, so they go first in the desc array
const allMessages = [
  ...pendingMessages,
  ...(data?.pages.flatMap(page => page.messages) ?? []),
];
```

When a message send completes (`done` SSE event):
1. Add the final Nova message to `pendingMessages`
2. Clear streaming state
3. Optionally invalidate the infinite query to sync server state (debounced, not blocking)

**Scroll behavior on new messages:** After the user sends a message and it is appended, scroll to bottom using `scrollRef.current.scrollTop = 0` (since `column-reverse` inverts the scroll axis).

The WebSocket subscription from `unify-conversation-streaming` (if applied) also appends to `pendingMessages` for cross-channel messages. This spec does not modify the WebSocket integration -- it preserves the append pattern.

#### Scenario: User sends message and receives streamed response

Given the user types a message and presses Enter,
when the message is sent via SSE to `/api/chat/send`,
then the user message appears immediately at the bottom (optimistic append),
and the streaming Nova response renders chunk-by-chunk in a `StreamingBubble`,
and on completion the final message replaces the streaming bubble,
and the view remains scrolled to the bottom.

#### Scenario: Cross-channel message arrives via WebSocket

Given a Telegram message is broadcast via the daemon's WebSocket,
when the dashboard receives the `message.complete` event,
then the message is appended to `pendingMessages`,
and it appears at the bottom of the chat without disrupting scroll position.

### Req-6: Delete Raw API Route

After the tRPC `message.chatHistory` procedure is verified, delete `apps/dashboard/app/api/chat/history/route.ts`. The chat page is the only consumer of this route. The existing `message.list` tRPC procedure (used by the `/messages` page) is not affected.

#### Scenario: No remaining consumers of the deleted route

Given `/api/chat/history` is only consumed by `apps/dashboard/app/chat/page.tsx`,
when the chat page is migrated to `trpc.message.chatHistory.infiniteQueryOptions()`,
then `apps/dashboard/app/api/chat/history/route.ts` is deleted,
and no other file imports or fetches from this path.

## Scope
- **IN**: New `message.chatHistory` tRPC cursor-based procedure, CSS `column-reverse` scroll container, `useInfiniteQuery` for paginated history, `IntersectionObserver` sentinel for upward loading, `pendingMessages` state for real-time appends, delete raw `/api/chat/history` route, remove `.reverse()` call
- **OUT**: WebSocket integration changes (preserved as-is), Telegram fallback changes (preserved as-is), SSE streaming changes (preserved as-is), message search, message grouping by date, "scroll to bottom" floating button (future enhancement), read receipts, typing indicators from other users

## Impact

| Area | Change |
|------|--------|
| `packages/api/src/routers/message.ts` | MODIFY -- add `chatHistory` cursor-based procedure |
| `apps/dashboard/app/chat/page.tsx` | MODIFY -- replace `apiFetch` with `useInfiniteQuery`, add `column-reverse` container, add `IntersectionObserver` sentinel, add `pendingMessages` state, remove `.reverse()`, remove `loadHistory` callback, remove `bottomRef` scroll anchor |
| `apps/dashboard/app/api/chat/history/route.ts` | DELETE -- replaced by tRPC procedure |

## Risks

| Risk | Mitigation |
|------|-----------|
| `column-reverse` scroll anchoring inconsistency across browsers | Chrome, Firefox, and Safari all support `flex-direction: column-reverse` with proper scroll anchoring. Dashboard is a known-browser environment (homelab, Chrome). Add `overflow-anchor: auto` as explicit declaration. |
| `useInfiniteQuery` page flattening creates stale data when new messages arrive | `pendingMessages` state is separate from the query cache. Periodic query invalidation (on focus, on new message send) keeps the cache fresh. The `pendingMessages` array is cleared when the query refetches and includes the latest messages. |
| Cursor-based pagination with `created_at` may have ties (same timestamp) | Messages within the same second are rare in a single-user assistant chat. If ties occur, the `+1` overfetch pattern still works correctly -- at worst, a message appears in two pages, and `key={msg.id}` deduplication in React prevents duplicate rendering. The UUID `id` column provides a stable identity. |
| Removing `.reverse()` changes the data contract for `allMessages` | The tRPC procedure returns descending order (newest first). The `column-reverse` container renders first-item-at-bottom. No other component consumes this data. The change is self-contained within the chat page. |
| SSE streaming appends to `pendingMessages` instead of the query cache | This is intentional -- SSE messages are ephemeral until the query refetches. On query invalidation, the server returns the latest messages including the ones just sent. `pendingMessages` is diffed against the latest page to avoid duplicates. |
| `IntersectionObserver` fires on initial render before messages load | The observer is only created when `hasNextPage` is true and the sentinel ref is mounted. On initial load, `isLoading` is true and the sentinel is not rendered, preventing premature triggers. |
