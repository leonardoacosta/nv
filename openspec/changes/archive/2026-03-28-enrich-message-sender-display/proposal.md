# Proposal: Enrich Message Sender Display

## Change ID
`enrich-message-sender-display`

## Summary

Replace raw sender IDs (e.g. `7380462766`) in the Messages page with resolved display names by enriching the `message.list` tRPC response with server-side contact resolution, Telegram API metadata extraction, and memory-based fallback. Update the dashboard UI to show sender avatars, channel badges, and resolved names without extra round-trips.

## Context
- Extends: `packages/api/src/routers/message.ts` (message.list query — currently returns raw `sender` string with no join), `apps/dashboard/app/messages/page.tsx` (Messages page with client-side `useContactResolver` hook)
- Related: `packages/api/src/routers/contact.ts` (contact.resolve mutation loads ALL contacts per call), `apps/dashboard/lib/entity-resolution/people-parser.ts` (parses memory `people` topic into PersonProfile[]), `apps/dashboard/lib/entity-resolution/contact-resolver.ts` (builds sender->name map from contacts + memory)
- Depends on: none (contacts table may be empty; resolution gracefully falls back)

## Motivation

The Messages page shows raw Telegram user IDs (`7380462766`) and platform identifiers instead of human-readable names. The existing resolution path has three problems:

1. **Client-side inefficiency** — `useContactResolver` fires a `contact.resolve` mutation per page load that fetches ALL contacts from the database regardless of which senders appear on screen. With thousands of contacts, this is O(n) per page load.
2. **No server-side join** — `message.list` returns the raw `sender` column with no attempt to resolve it. Every client must independently resolve senders.
3. **Untapped metadata** — Telegram messages store `first_name`, `last_name`, and `username` in the `metadata` JSONB field, but this data is never surfaced. For senders not in the contacts table, this is the best available display name.

By moving resolution server-side into the tRPC response, every consumer gets resolved names for free, the client-side resolve call is eliminated, and Telegram metadata provides a zero-config fallback before resorting to memory heuristics.

## Requirements

### Req-1: Server-side sender resolution in message.list

Enrich the `message.list` tRPC response with a `senderResolved` object per message containing `displayName`, `avatarInitial`, and `source` (indicating where the name came from). Resolution priority:

1. **Contacts table** — LEFT JOIN messages to contacts via `contacts.channel_ids` JSONB matching `messages.channel` + `messages.sender`
2. **Telegram metadata** — Extract `first_name`/`last_name`/`username` from `messages.metadata` JSONB when `channel = 'telegram'`
3. **Memory people profiles** — Query the `memory` table for the `people` topic, parse it server-side using the existing `parsePeopleMemory()` logic, and match by channel ID
4. **Raw fallback** — If none of the above resolve, return the raw sender string

The resolution must be batched: collect all unique senders from the current page, resolve them in bulk, then attach results to each message. Do not run per-row subqueries.

### Req-2: Extend StoredMessage response type

Add the following fields to the message.list response objects:

```typescript
{
  // existing fields...
  senderResolved: {
    displayName: string;      // resolved human name or raw sender
    avatarInitial: string;    // first character of displayName, uppercased
    source: "contact" | "telegram-meta" | "memory" | "raw";
  }
}
```

Update `StoredMessage` in `apps/dashboard/types/api.ts` to include the new `senderResolved` field.

### Req-3: Extract Telegram metadata helper

Create a helper function that extracts display name from Telegram message metadata JSONB:

```typescript
function extractTelegramName(metadata: Record<string, unknown> | null): string | null
```

Expected metadata shapes (from Telegram Bot API `Message.from`):
- `{ from: { first_name: "Leo", last_name: "N", username: "leonyaptor" } }`
- `{ first_name: "Leo", username: "leonyaptor" }` (flat)

Returns `"first_name last_name"` (trimmed), falling back to `username`, falling back to `null`.

### Req-4: Move people-parser to API package

Move `parsePeopleMemory()` from `apps/dashboard/lib/entity-resolution/people-parser.ts` to `packages/api/src/lib/people-parser.ts` so the server-side resolver can use it. The dashboard can import from the tRPC response instead of running its own parsing. Keep the existing dashboard file as a re-export for backward compatibility if other dashboard code uses it.

### Req-5: Update Messages page to use server-resolved names

Replace the `useContactResolver` hook with direct use of `senderResolved` from the tRPC response:

- Remove the `useContactResolver` hook and the `trpcClient.contact.resolve.mutate` call
- Read `msg.senderResolved.displayName` directly for the sender column
- Use `msg.senderResolved.avatarInitial` to render a circular avatar initial badge (single character, colored by channel accent)
- In the expanded message detail, show the resolution source as a subtle label (e.g. "via contacts", "via Telegram profile", "via memory")

### Req-6: Sender avatar initial and channel badge

Add a compact avatar element to each message row:
- Circular 20px badge showing `avatarInitial` in the channel's accent color (background) with white text
- For outbound messages (sender = "nova"), show a distinct "N" badge with a fixed brand color
- Position before the sender name in the dense row layout, replacing the bare direction arrow for inbound messages

## Scope
- **IN**: Server-side sender resolution in message.list, Telegram metadata extraction, memory people-parser on API side, StoredMessage type extension, avatar initial badges, removal of client-side useContactResolver
- **OUT**: Contact creation/editing UI, contacts table population (separate dual-DB fix), profile photos/images, real-time sender updates via WebSocket, changes to message ingestion pipeline, changes to other pages that use contact resolution

## Impact
| Area | Change |
|------|--------|
| `packages/api/src/routers/message.ts` | Modified: add bulk sender resolution (contacts JOIN, metadata extraction, memory fallback) to message.list query |
| `packages/api/src/lib/people-parser.ts` | New: moved from dashboard entity-resolution |
| `packages/api/src/lib/sender-resolver.ts` | New: batched resolver function used by message.list |
| `apps/dashboard/types/api.ts` | Modified: add `senderResolved` field to `StoredMessage` |
| `apps/dashboard/app/messages/page.tsx` | Modified: remove useContactResolver hook, use senderResolved from tRPC, add avatar initials |
| `apps/dashboard/lib/entity-resolution/people-parser.ts` | Modified: re-export from API package |

## Risks
| Risk | Mitigation |
|------|-----------|
| Contacts table is currently empty — resolution always falls back | Telegram metadata provides names for most senders; memory fallback covers the rest. Resolution degrades gracefully: worst case is the same raw ID currently shown. |
| Memory `people` topic query adds latency to message.list | Cache the parsed people profiles for the duration of the request (single DB query + parse). The people topic is typically <10KB. |
| Telegram metadata schema varies between bot types | `extractTelegramName` checks both nested (`from.first_name`) and flat (`first_name`) shapes, returning null on any unexpected structure. |
| LEFT JOIN to contacts via JSONB is slow | The join uses a SQL expression matching `contacts.channel_ids->>channel = sender`. With <1000 contacts this is fast. If contacts grow, add a GIN index on `channel_ids`. |
| Moving people-parser to API package breaks dashboard imports | Keep dashboard file as a thin re-export; no breaking change for existing consumers. |
