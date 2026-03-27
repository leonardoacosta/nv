# Proposal: Messages Dense View

## Change ID
`messages-dense-view`

## Summary

Rebuild the Messages page as a high-density log viewer with virtual scrolling, keyboard navigation, faceted filters, contact name resolution, and message type badges -- modeled after Vercel deployment log density and benchmarked against the Diary page.

## Context
- Extends: `apps/dashboard/app/messages/page.tsx` (Messages page with hour grouping, channel filters, search, Prev/Next pagination, expandable rows)
- Extends: `apps/dashboard/app/api/messages/route.ts` (REST endpoint returning paginated `StoredMessage[]`)
- Related: `improve-messages-ux` (shipped -- added hour grouping, inline expand, channel color pills)
- Related: `global-density-pass` (shipped -- tightened spacing, removed max-width, inline stats)
- Related: `apps/dashboard/lib/channel-colors.ts` (deterministic accent color hashing)
- Related: `packages/db/src/schema/contacts.ts` (contacts table with `channelIds` JSONB for entity resolution)
- Related: `packages/db/src/schema/messages.ts` (messages table with `channel`, `sender`, `content`, `metadata` columns)

## Motivation

The Messages page received visual polish (hour grouping, channel color pills, inline expand) but the underlying interaction model remains basic. It functions as a paginated table rather than a purpose-built log viewer. Specific gaps:

1. **Row density is too low** -- each message row uses `py-3` padding, a 3px accent border, full channel badge, and a separate "Show more" toggle region. A single message consumes 44-60px of vertical height. The Diary page achieves 28-36px per entry row. For a log viewer handling hundreds of messages, this density gap compounds into excessive scrolling.

2. **Pagination blocks flow** -- Prev/Next pagination forces discrete page loads. A log viewer should stream continuously as the user scrolls. The current API already supports `limit`/`offset`, making cursor-based infinite scroll straightforward.

3. **Raw sender IDs are opaque** -- `sender` contains raw channel identifiers like `telegram:7380462766`. The contacts table already stores `channelIds` JSONB that maps these identifiers to contact names. This data exists but is not surfaced on the messages page.

4. **No keyboard navigation** -- log viewers are keyboard-driven tools. There is no way to move through messages with j/k or expand with Enter. Users must click each row individually.

5. **Filters are spatially separated** -- channel pills, date range chips, and the search box occupy separate rows in the controls bar. A unified faceted filter strip would reduce vertical overhead and make the active filter state scannable at a glance.

6. **No sort flexibility** -- messages are always newest-first. There is no option to sort by channel or direction, which would be useful for reviewing all outbound messages or all messages from a specific channel in sequence.

7. **No message type differentiation** -- tool calls, system messages, and conversation messages all render identically. A small badge indicating message type would help users filter signal from noise.

8. **No direction filtering** -- the direction icons (inbound/outbound) exist visually but there is no way to filter by direction.

## Requirements

### Req-1: Compact Message Rows

Reduce message row height to 28-32px (matching Diary density). Each row renders on a single line: `[direction-icon] [channel-icon] [resolved-name | sender] [message-preview] [type-badge] [latency] [timestamp]`. Remove the separate "Show more" region from collapsed state -- expanding happens via click or Enter key on the row itself. The 3px left accent border is retained. Padding reduces from `py-3 px-4` to `py-1.5 px-3`. Font sizes reduce to `text-xs` / `text-[11px]` for metadata, `text-sm` for message preview.

### Req-2: Virtual Scrolling

Replace Prev/Next pagination with infinite scroll using a virtual list. The container renders a fixed viewport with overscan, only mounting visible rows plus a buffer. As the user scrolls toward the bottom, fetch the next page from `/api/messages?limit=50&offset=N`. Use `@tanstack/react-virtual` (already in the monorepo dependency tree via TanStack Query) or a lightweight virtualizer. Retain hour-group dividers as sticky headers within the virtual list. The initial load fetches 50 messages; subsequent pages append. A "scroll to top" button appears after scrolling past the first fold.

### Req-3: Contact Name Resolution

Add a `/api/contacts/resolve` endpoint (or extend the existing `/api/contacts` route) that accepts an array of sender identifiers and returns a `Record<string, string>` mapping sender IDs to contact names. The endpoint queries the `contacts` table, scanning `channelIds` JSONB for matches. On the frontend, after fetching a page of messages, extract unique sender values and resolve them in a single batch call. Cache resolved names in a `Map<string, string>` across pages. Display the resolved name in the row; fall back to the raw sender if no contact match exists. Format: "Leo" not "telegram:7380462766".

### Req-4: Keyboard Navigation

When the message list has focus, j/k moves the active row highlight down/up. Enter expands the active row (same as click). Escape collapses an expanded row and returns focus to the list. The active row is visually indicated with a subtle background highlight (`bg-ds-gray-alpha-100`). Arrow keys also work as aliases for j/k. Focus management: clicking a row or pressing Tab into the list activates keyboard mode; clicking outside deactivates it.

### Req-5: Faceted Filter Bar

Consolidate channel pills, direction filter, date range, and search into a single horizontal filter strip. Layout: `[search-input] [channel-dropdown] [direction-dropdown] [date-range-toggle] [sort-dropdown]`. Each filter is a compact dropdown or toggle, not a row of pills. Active filters show a count badge. A "Clear all" button appears when any filter is active. The filter strip occupies a single row, max 40px height.

### Req-6: Direction Filter

Add an "inbound" / "outbound" / "all" toggle to the faceted filter bar. When active, client-side filter the displayed messages (the API does not currently support direction filtering since direction is derived at response time from `sender === "nova"`). Show the active direction as a pill in the filter bar.

### Req-7: Sort Options

Add a sort dropdown to the filter bar with options: "Newest first" (default, current behavior), "Oldest first" (reverse chronological), "By channel" (group by channel, then chronological within each), "By direction" (inbound first, then outbound, chronological within each). Sorting is client-side on the fetched messages. When sorted by channel or direction, hour-group dividers are replaced with channel/direction group headers.

### Req-8: Message Type Badges

Add a small inline badge to each message row indicating type: `conversation` (default, no badge -- clean), `tool-call` (orange "tool" badge), `system` (gray "sys" badge). Type is derived from the `metadata` JSONB field on the messages table. If `metadata` contains a `type` key, use it; otherwise default to `conversation`. Badges use `text-[10px]` font, `px-1 py-0.5 rounded` styling, monospace font.

### Req-9: API Enhancements

Extend `GET /api/messages` to support:
- `direction` query param: filter by "inbound" or "outbound" (server-side, using `sender = 'nova'` for outbound)
- `type` query param: filter by message type from metadata JSONB
- `sort` query param: "asc" or "desc" (default "desc") for timestamp ordering
- Return `total` count in the response (add a `COUNT(*)` query) so the frontend can show "Showing N of M messages"

Extend the response type:
```typescript
export interface MessagesGetResponse {
  messages: StoredMessage[];
  total: number;
  limit: number;
  offset: number;
}
```

Add `type` field to `StoredMessage`:
```typescript
export interface StoredMessage {
  // ...existing fields...
  type: "conversation" | "tool-call" | "system";
}
```

## Scope
- **IN**: Row density reduction, virtual scrolling, contact name resolution endpoint + client cache, keyboard navigation (j/k/Enter/Escape), faceted filter bar, direction filter, sort options, message type badges, API enhancements (direction/type/sort params, total count)
- **OUT**: Real-time WebSocket message streaming, message sending/composing, conversation threading, full-text search improvements (existing search is sufficient), changes to the messages DB schema (we read `metadata` JSONB as-is), message deletion or editing

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/messages/page.tsx` | Major rewrite: compact rows, virtual scroll, keyboard nav, faceted filters, sort, type badges |
| `apps/dashboard/app/api/messages/route.ts` | Extended: direction/type/sort params, total count, type extraction from metadata |
| `apps/dashboard/app/api/contacts/resolve/route.ts` | New: batch sender-to-name resolution endpoint |
| `apps/dashboard/types/api.ts` | Modified: add `type` to `StoredMessage`, add `total` to `MessagesGetResponse` |
| `apps/dashboard/lib/channel-colors.ts` | No change (reused as-is) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Virtual scroll may introduce visual jank with variable-height expanded rows | Use dynamic row measurement via `@tanstack/react-virtual`'s `measureElement`; expanded rows re-measure on toggle. Limit overscan to 10 rows. |
| Contact resolution adds a network round-trip per page | Batch resolve in single call; cache results in `Map` across page loads; contacts change infrequently so stale reads are acceptable. |
| JSONB scan for `channelIds` matching may be slow with many contacts | Contact table is small (tens of rows); a full scan of JSONB is acceptable. If scale increases, add a GIN index on `channel_ids`. |
| Keyboard navigation may conflict with browser shortcuts | Only activate when the message list container has focus; use `event.preventDefault()` only for j/k/Enter/Escape within the list. |
| Faceted filter dropdown may feel cramped on mobile | Filter bar wraps on small screens; dropdowns use full-width mobile sheets. Acceptable degradation since this is primarily a desktop tool. |
| Message type derivation from metadata JSONB may not cover all cases | Default to "conversation" when metadata is null or missing `type` key. This is safe -- most messages are conversations. |
