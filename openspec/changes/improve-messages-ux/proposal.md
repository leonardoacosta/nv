# Proposal: Improve Messages UX

## Change ID
`improve-messages-ux`

## Summary

Improve Messages page readability by grouping messages by time window, adding expandable long messages, and color-coding channels.

## Context
- Extends: `apps/dashboard/app/messages/page.tsx` (Messages page component with flat message list, channel filter tabs, search, pagination)
- Related: existing `CHANNEL_COLOR` and `CHANNEL_ICONS` maps in the page already assign colors to known channels; this spec generalizes color assignment to dynamic/unknown channels and applies it more broadly

## Motivation

Messages display as a flat log with no visual hierarchy. Long messages truncate at 120 characters via `truncate()` without an expand affordance — the only way to read the full content is to click-expand the entire row, which shows metadata alongside the content. Channel tabs are plain text with no visual differentiation beyond a small icon. This makes the page hard to scan and reduces its value as a conversation history view.

1. **Time grouping** — consecutive messages from the same hour blend together with no temporal anchors, making it difficult to orient within a conversation timeline.
2. **Expandable long messages** — the current truncation at 120 chars with no inline expand means users must open the full detail panel for every long message.
3. **Channel color coding** — the existing `CHANNEL_COLOR` map only covers 5 known channels. Dynamic channels get a fallback gray. A deterministic hash-based palette ensures every channel gets a consistent, distinct accent.
4. **Channel filter pills** — the filter tabs use a uniform gray style regardless of channel, missing an opportunity to reinforce channel identity.

## Requirements

### Req-1: Time Grouping

Group messages by hour (or by conversation thread if available) with subtle time dividers between groups. Each divider shows the date and hour range (e.g., "Today, 2:00 PM - 3:00 PM" or "Mar 25, 10:00 AM - 11:00 AM"). Groups are derived client-side from the existing `timestamp` field on `StoredMessage`.

### Req-2: Expandable Long Messages

Truncate messages beyond 3 lines with a "Show more" toggle. Clicking "Show more" expands the message content inline with a smooth CSS height transition (not the full metadata panel). The toggle text changes to "Show less" when expanded. This is independent of the existing row expand/collapse which shows full metadata.

### Req-3: Channel Color Coding

Assign each channel a deterministic accent color derived by hashing the channel name into a curated palette of 8 high-contrast colors. Known channels (telegram, discord, slack, cli, api) retain their existing brand colors. Unknown/dynamic channels cycle through the palette. Apply the accent as a 3px left border on each message row and as a subtle background tint on the channel badge.

### Req-4: Channel Filter Pills

Upgrade the plain text filter tabs to pill-style buttons with the channel's accent color applied as a left border or background tint when active. Inactive pills remain neutral. The "All channels" pill uses a neutral style.

## Scope
- **IN**: Message time grouping with dividers, inline expand/collapse for long messages, deterministic channel color hashing, channel filter pill styling
- **OUT**: Message sending, search enhancements, real-time WebSocket message arrival animations, changes to the message data model or API, pagination logic changes

## Impact
| Area | Change |
|------|--------|
| `apps/dashboard/app/messages/page.tsx` | Modified: add time group dividers, inline expand/collapse for message content, channel color hashing, pill-style channel filters |

## Risks
| Risk | Mitigation |
|------|-----------|
| Color hashing may produce poor contrast against the dark dashboard theme | Use a curated palette of 8 colors tested against `ds-gray-100` and `ds-gray-200` backgrounds; known channels keep their brand colors |
| Height animation on expand may cause layout jank with many messages | Use CSS `max-height` transition with `overflow: hidden` rather than JS-driven animation; keep transition duration short (200ms) |
| Time grouping logic adds client-side computation per render | Groups are computed via a single `O(n)` pass over the already-fetched page of messages (max 50); negligible cost |
