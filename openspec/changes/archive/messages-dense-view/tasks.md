# Implementation Tasks

## API Batch

- [x] [1.1] [P-1] Create `apps/dashboard/app/api/contacts/resolve/route.ts` -- new POST endpoint accepting `{ senders: string[] }` body; query the `contacts` table, scan `channelIds` JSONB for matches against each sender identifier; return `Record<string, string>` mapping sender IDs to contact display names; return 200 with empty object if no matches [owner:api-engineer]
- [x] [1.2] [P-1] Extend `apps/dashboard/app/api/messages/route.ts` -- add `direction` query param (filter using `sender = 'nova'` for outbound, `sender != 'nova'` for inbound); add `sort` param ("asc"/"desc", default "desc"); add `type` param (filter on `metadata->>'type'`); add a parallel `COUNT(*)` query and return `total` in the response; extract `metadata.type` into the mapped response as `type` field defaulting to `"conversation"` [owner:api-engineer]
- [x] [1.3] [P-2] Update `apps/dashboard/types/api.ts` -- add `type: "conversation" | "tool-call" | "system"` field to `StoredMessage` interface; add `total: number` to `MessagesGetResponse` [owner:api-engineer]

## UI Batch 1 -- Core Components

- [x] [2.1] [P-1] Create compact `MessageRowDense` component in `apps/dashboard/app/messages/page.tsx` -- single 28-32px row: direction icon (13px, emerald inbound / amber outbound) + channel icon (13px) + resolved sender name or raw sender (text-xs, 80px truncated) + message preview (text-sm, flex-1 truncate) + type badge (only for tool-call/system, text-[10px] mono) + latency badge (if present, text-[10px] mono) + timestamp (text-xs mono, right-aligned); reduce padding to `py-1.5 px-3`; retain 3px left accent border from `channelAccentColor`; active row state via `bg-ds-gray-alpha-100` highlight [owner:ui-engineer]
- [x] [2.2] [P-1] Add virtual scroll container -- replace the `<ul>` message list and `PaginationControls` with a virtualizer using `@tanstack/react-virtual`; fixed viewport height (`calc(100vh - header - filters)`); row estimateSize 30px; dynamic measurement via `measureElement` for expanded rows; overscan 10 rows; infinite load: trigger fetch of next 50 messages when scroll position reaches last 5 rows; append new messages to existing array; retain hour-group sticky dividers as non-interactive virtual rows [owner:ui-engineer]
- [x] [2.3] [P-1] Add contact name resolution hook -- create `useContactResolver` hook that accepts `StoredMessage[]`, extracts unique sender values, calls `POST /api/contacts/resolve` in a batch, caches results in a `useRef(new Map<string, string>())` across fetches; returns `resolve(sender: string): string` function that returns display name or raw sender fallback; skip re-fetching senders already in cache [owner:ui-engineer]

## UI Batch 2 -- Filters + Navigation

- [x] [3.1] [P-1] Build faceted filter bar -- replace the current multi-row controls (search input, channel pills, date range chips) with a single-row filter strip: search input (flex-1, max-w-xs) + channel dropdown (compact select, shows channel icon + name, "All" default) + direction dropdown ("All" / "Inbound" / "Outbound") + date range toggle (Today / 7d / All, segmented control) + sort dropdown (Newest / Oldest / By channel / By direction); max 40px height; "Clear all" button appears when any non-default filter is active [owner:ui-engineer]
- [x] [3.2] [P-2] Add keyboard navigation -- attach `onKeyDown` handler to the virtual scroll container; j/ArrowDown moves active index +1, k/ArrowUp moves -1; Enter toggles expand on active row; Escape collapses expanded row; active index tracked in `useState`; scroll active row into view via virtualizer `scrollToIndex`; visual indicator: active row gets `bg-ds-gray-alpha-100` ring; only active when container has focus (tabIndex=0) [owner:ui-engineer]
- [x] [3.3] [P-2] Add message type badges -- in `MessageRowDense`, render a small badge after the message preview for non-conversation types: `tool-call` shows orange badge "tool" (`bg-amber-500/15 text-amber-500`), `system` shows gray badge "sys" (`bg-ds-gray-alpha-200 text-ds-gray-900`); `conversation` type renders no badge (clean default) [owner:ui-engineer]
- [x] [3.4] [P-2] Add sort logic -- when sort is "Newest" or "Oldest", messages maintain chronological order (API handles via `sort` param); when sort is "By channel", group messages by channel name (alphabetical), chronological within each group, replace hour dividers with channel group headers; when sort is "By direction", group by inbound/outbound, chronological within each, replace hour dividers with direction group headers [owner:ui-engineer]
- [x] [3.5] [P-3] Add "scroll to top" button -- floating button appears after scrolling past first 20 rows; positioned bottom-right of the message list container; clicking scrolls to index 0 via virtualizer; uses `ArrowUp` icon, `surface-card` styling, 32px circle [owner:ui-engineer]

## Verify

- [x] [4.1] `cd apps/dashboard && pnpm typecheck` passes -- zero TypeScript errors [owner:ui-engineer]
- [ ] [4.2] `cd apps/dashboard && pnpm build` passes -- no build errors [owner:ui-engineer]
- [ ] [4.3] [user] Visual review: message rows are 28-32px height, matching Diary page density
- [ ] [4.4] [user] Visual review: virtual scroll loads smoothly with no visible jank when scrolling through 200+ messages
- [ ] [4.5] [user] Visual review: sender names resolve to contact display names (e.g., "Leo" instead of "telegram:7380462766")
- [ ] [4.6] [user] Visual review: j/k navigation moves highlight through rows, Enter expands, Escape collapses
- [ ] [4.7] [user] Visual review: faceted filter bar fits in a single row with all controls accessible
- [ ] [4.8] [user] Visual review: type badges appear on tool-call and system messages, no badge on conversation messages
- [ ] [4.9] [user] Visual review: sort by channel groups messages under channel headers, sort by direction groups under direction headers
