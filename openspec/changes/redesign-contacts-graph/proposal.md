# Proposal: Redesign Contacts as Auto-Populated Relationship Graph

## Change ID
`redesign-contacts-graph`

## Summary

Replace the manual address-book contacts page with an auto-populated relationship view built from
Nova's conversations. Contacts are discovered by aggregating unique senders from the messages table
(SQLite) across all channels (Telegram, Discord, Teams), enriched with metadata from Nova's memory
profiles, and displayed as rich profile cards with relationship badges instead of an empty CRUD list.

## Context

- Phase: Dashboard redesign (post-v10)
- Dependencies: None -- all data sources exist (messages.db, contacts table, memory/people.md, config/contact/)
- Key sources:
  - `crates/nv-daemon/src/contact_store.rs` -- Rust SQLite CRUD for contacts table (v9 migration)
  - `crates/nv-daemon/src/messages.rs` -- `MessageStore` with `log_inbound()` storing sender + channel + contact_id FK (v10 migration)
  - `crates/nv-daemon/src/http.rs` -- Axum HTTP routes: `GET/POST /api/contacts`, `GET /api/messages`
  - `dashboard/src/pages/ContactsPage.tsx` -- current Vite SPA contacts page (manual CRUD, 633 lines)
  - `dashboard/src/components/Sidebar.tsx` -- sidebar nav with `NAV_ITEMS` array
  - `packages/db/src/schema/contacts.ts` -- Drizzle Postgres schema (id, name, channelIds, relationshipType, notes)
  - `packages/db/src/schema/messages.ts` -- Drizzle Postgres schema (id, channel, sender, content, metadata, createdAt)
  - `memory/people.md` -- Nova's memory file with detailed people profiles (~14KB, 30+ people)
  - `config/contact/example-contact.md` -- contact profile template for system context injection

## Motivation

The current contacts page is a manual address book: empty by default, requires the user to
create/edit/delete entries one by one. This is backwards for an AI assistant that talks to dozens of
people daily across multiple channels. Nova already knows who she talks to -- the messages table has
every sender, every channel, every timestamp. The contacts page should reflect that reality:

- **Auto-discovery**: Parse existing messages to find everyone Nova has talked to, without manual entry
- **Rich profiles**: Show interaction frequency, active channels, last seen, role/title from memory
- **Relationship context**: Show how people relate to each other via shared channels and co-mentions
- **Recency-first**: Sort by last interaction, not alphabetically -- show who matters right now

## Design

### Data Model: Discovery vs. Stored Contacts

The system operates on two tiers of contact data:

**Tier 1 -- Discovered Contacts** (computed from messages table):
- Aggregated from `SELECT DISTINCT sender, channel FROM messages WHERE direction = 'inbound' AND sender IS NOT NULL AND sender != ''`
- Yields: sender name, channels active on, message count, first/last message timestamps
- This is the primary data source -- every person who has messaged Nova appears automatically

**Tier 2 -- Enriched Contacts** (stored in contacts table):
- The existing contacts table stores curated metadata: relationship_type, notes, channel_ids mapping
- When a discovered contact matches a stored contact (by channel_ids JSON lookup or name match),
  the stored metadata enriches the discovered profile
- Nova can enrich contacts via memory writes or the operator can edit profiles in config/contact/

**Tier 3 -- Memory Profiles** (from memory/people.md and config/contact/):
- Nova's memory file contains detailed profiles with role, style, projects, timezone
- Contact profile .md files in config/contact/ are injected into Nova's system context
- These provide the richest metadata but are unstructured text -- used for display only

### Backend: New Discovery Endpoint

Add a new `GET /api/contacts/discovered` endpoint to the Rust daemon that:

1. Queries the messages table for unique inbound senders grouped by (sender, channel)
2. Aggregates per-sender: total message count, distinct channels, first_seen, last_seen
3. Left-joins with the contacts table to merge stored metadata (relationship_type, notes)
4. Returns a unified list sorted by last_seen DESC

The query (SQLite):
```sql
SELECT
  m.sender AS name,
  GROUP_CONCAT(DISTINCT m.channel) AS channels,
  COUNT(*) AS message_count,
  MIN(m.timestamp) AS first_seen,
  MAX(m.timestamp) AS last_seen,
  c.id AS contact_id,
  c.relationship_type,
  c.notes,
  c.channel_ids
FROM messages m
LEFT JOIN contacts c ON m.contact_id = c.id
WHERE m.direction = 'inbound'
  AND m.sender IS NOT NULL
  AND m.sender != ''
  AND m.sender != 'nova'
GROUP BY m.sender
ORDER BY last_seen DESC
```

Response shape:
```json
{
  "contacts": [
    {
      "name": "Therese Lay",
      "channels": ["teams"],
      "message_count": 847,
      "first_seen": "2025-11-15T09:23:00",
      "last_seen": "2026-03-26T14:30:00",
      "contact_id": "uuid-or-null",
      "relationship_type": "work",
      "notes": "PM for Fireball team",
      "channel_ids": {"teams": "therese.lay@bbins.com"}
    }
  ],
  "total_senders": 42,
  "total_messages_scanned": 12847
}
```

### Backend: Relationship Data Endpoint

Add `GET /api/contacts/relationships` that returns edge data for the relationship view:

```sql
SELECT
  a.sender AS person_a,
  b.sender AS person_b,
  a.channel AS shared_channel,
  COUNT(*) AS co_occurrence_count
FROM messages a
JOIN messages b ON a.channel = b.channel
  AND date(a.timestamp) = date(b.timestamp)
  AND a.sender < b.sender
WHERE a.direction = 'inbound'
  AND b.direction = 'inbound'
  AND a.sender IS NOT NULL AND a.sender != '' AND a.sender != 'nova'
  AND b.sender IS NOT NULL AND b.sender != '' AND b.sender != 'nova'
GROUP BY a.sender, b.sender, a.channel
HAVING co_occurrence_count >= 3
ORDER BY co_occurrence_count DESC
```

This finds people who message on the same channel on the same day -- a proxy for "these people
are connected" without needing explicit relationship tracking.

### Frontend: Contacts Page Redesign

The page has three sections:

**Header Stats Bar**:
- "Discovered N contacts from M conversations" subtitle
- Search input (filters by name)
- Relationship type filter tabs (All, Work, Personal Client, Contributor, Social, Untagged)

**Contact Cards Grid** (replaces the flat list):
Each card shows:
- Name (large, bold)
- Channel badges (Telegram, Discord, Teams icons with accent colors)
- Message count + last interaction relative time ("847 messages, last seen 2h ago")
- Relationship badge (Work/Contributor/Social/Untagged)
- Role/title line (from contacts.notes or memory if available)
- Related people list (from relationship endpoint -- "Also talks with: James, Rickey, Kirk")

Cards are sorted by last interaction (most recent first). Clicking a card opens a detail
drawer/panel (not a modal) showing full profile info.

**Contact Detail Panel** (slide-in from right):
- Full name, all channel identifiers
- Interaction timeline sparkline (messages per week over last 3 months)
- Channel breakdown (pie/bar showing message distribution across channels)
- Related people with shared channel badges
- Notes section (from contacts table, read-only in this spec)
- Link to memory topic if available

### Removed Features

- **"New Contact" button**: Removed. Contacts are auto-discovered, not manually created.
- **Edit/Delete actions**: Removed from the card view. Contact enrichment happens through Nova's
  memory or config/contact/ profile files, not through the dashboard UI.
- **Contact Modal**: The create/edit modal is removed entirely.

### Design System

The page uses the existing cosmic design tokens (cosmic-dark, cosmic-surface, cosmic-border,
cosmic-purple, cosmic-bright, cosmic-muted, cosmic-text) already established across all dashboard
pages. Channel badges reuse the existing `ChannelBadges` component pattern from the current page.

## Requirements

### Req-1: Discovery Endpoint in Rust Daemon

Add `GET /api/contacts/discovered` to `http.rs`:
- New struct `DiscoveredContact` with fields: name, channels (Vec<String>), message_count (i64),
  first_seen (String), last_seen (String), contact_id (Option<String>), relationship_type
  (Option<String>), notes (Option<String>), channel_ids (Option<serde_json::Value>).
- New struct `DiscoveredContactsResponse` with fields: contacts (Vec<DiscoveredContact>),
  total_senders (i64), total_messages_scanned (i64).
- New query method `discover_contacts()` on `MessageStore` that executes the aggregation query
  joining messages and contacts tables.
- Register route in the Axum router alongside existing `/api/contacts` routes.

### Req-2: Relationships Endpoint in Rust Daemon

Add `GET /api/contacts/relationships` to `http.rs`:
- New struct `ContactRelationship` with fields: person_a (String), person_b (String),
  shared_channel (String), co_occurrence_count (i64).
- New struct `RelationshipsResponse` with fields: relationships (Vec<ContactRelationship>).
- New query method `discover_relationships()` on `MessageStore` that executes the co-occurrence
  query.
- Optional query param `?min_count=N` to filter edges (default 3).
- Register route in the Axum router.

### Req-3: Dashboard TypeScript Types

Add to `dashboard/src/types/api.ts`:
- `DiscoveredContact` interface matching the Rust struct
- `DiscoveredContactsResponse` interface
- `ContactRelationship` interface
- `RelationshipsResponse` interface

### Req-4: Contacts Page Full Rewrite

Replace `dashboard/src/pages/ContactsPage.tsx` entirely:
- Remove all CRUD logic (createContact, updateContact, deleteContact, ContactModal, DeleteConfirm)
- State: `contacts: DiscoveredContact[]`, `relationships: ContactRelationship[]`, `loading: boolean`,
  `error: string | null`, `search: string`, `filterTab: FilterTab`, `selectedContact: DiscoveredContact | null`
- On mount: fetch `GET /api/contacts/discovered` and `GET /api/contacts/relationships` in parallel
- Header: title "Contacts", subtitle "Discovered N contacts from M conversations"
- Search: filters contacts by name (client-side filter on fetched data)
- Filter tabs: All, Work, Personal Client, Contributor, Social, Untagged (where Untagged = no
  relationship_type). Reuse existing `FILTER_TABS` pattern but add "Untagged" tab.
- Contact cards: grid layout (responsive: 1 col mobile, 2 col tablet, 3 col desktop)
- Sort: by last_seen DESC (most recent first)
- Click card: set `selectedContact`, show detail panel

### Req-5: Contact Card Component

New component in `dashboard/src/components/ContactCard.tsx`:
- Props: `contact: DiscoveredContact`, `relatedPeople: string[]`, `onClick: () => void`
- Layout: cosmic-surface card with border, hover state (border-cosmic-purple/50)
- Shows: name, channel badges, message count, relative last_seen time, relationship pill,
  truncated notes, "Also talks with: ..." list (max 3 names, "+N more")
- Message count formatted with locale string (e.g., "1,234 messages")
- Last seen as relative time ("2h ago", "3d ago", "2w ago") -- computed client-side

### Req-6: Contact Detail Panel

New component in `dashboard/src/components/ContactDetailPanel.tsx`:
- Props: `contact: DiscoveredContact`, `relationships: ContactRelationship[]`, `onClose: () => void`
- Slide-in panel from right side, overlay on top of the grid
- Full width on mobile, 400px fixed width on desktop
- Shows: full name, all channel identifiers with copy button, message count, first/last seen
  dates (absolute + relative), relationship type, full notes text, related people list with
  shared channel info and co-occurrence counts
- Close button (X) in top-right corner
- Backdrop click closes

### Req-7: Relative Time Utility

New utility function in `dashboard/src/pages/ContactsPage.tsx` (inline, not a separate file):
- `function relativeTime(iso: string): string` -- converts ISO timestamp to human-readable
  relative time ("just now", "5m ago", "2h ago", "3d ago", "2w ago", "3mo ago", ">1y ago")
- Used by ContactCard and ContactDetailPanel

## Scope

**IN**: Discovery endpoint, relationships endpoint, TS types, full page rewrite with card grid,
contact detail panel, relationship badges, channel badges, search/filter, recency sort.

**OUT**: Graph visualization (deferred -- start with grouped cards + relationship badges), contact
editing from dashboard, importing from external sources, memory/people.md parsing into structured
data, contact merge/dedup logic, notification when new contacts are discovered.

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/messages.rs` | Add `discover_contacts()` and `discover_relationships()` query methods |
| `crates/nv-daemon/src/http.rs` | Add `GET /api/contacts/discovered` and `GET /api/contacts/relationships` routes + handler functions |
| `dashboard/src/types/api.ts` | Add `DiscoveredContact`, `DiscoveredContactsResponse`, `ContactRelationship`, `RelationshipsResponse` types |
| `dashboard/src/pages/ContactsPage.tsx` | Full rewrite: remove CRUD, add discovery view with card grid + detail panel |
| `dashboard/src/components/ContactCard.tsx` | New: contact card component for grid layout |
| `dashboard/src/components/ContactDetailPanel.tsx` | New: slide-in detail panel component |

## Risks

| Risk | Mitigation |
|------|-----------|
| Discovery query is slow on large message tables | Add index `idx_messages_sender_direction` on `(sender, direction)`. The query only scans inbound messages. For 10K+ messages, response time should be <200ms with index. |
| Sender names are inconsistent across channels (e.g., "Leo" on Telegram vs "Leo Acosta" on Teams) | The contact_id FK (v10 migration) already links messages to canonical contacts. Discovery endpoint groups by sender name but merges via contact_id when available. Full dedup is out of scope. |
| Relationship co-occurrence query is O(n^2) on same-day messages | The HAVING clause (min 3 co-occurrences) filters noise. Query is paginated server-side. For initial launch, the relationship data is secondary -- page works without it. |
| Memory/people.md is unstructured text, hard to parse | Out of scope for this spec. Notes from the contacts table are the structured enrichment source. Memory parsing is a future enhancement. |
| Removing CRUD breaks existing workflow | The existing contacts page has near-zero usage (contacts are empty by default). Auto-discovery is strictly better. Contact enrichment via config/contact/ profile files and Nova's memory remains available. |
