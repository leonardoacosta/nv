# Implementation Tasks

## DB Batch: Discovery Queries

- [ ] [1.1] [P-1] Add `discover_contacts()` method to `MessageStore` in `crates/nv-daemon/src/messages.rs` -- executes aggregation query: SELECT sender, GROUP_CONCAT(DISTINCT channel), COUNT(*), MIN(timestamp), MAX(timestamp) FROM messages WHERE direction='inbound' AND sender IS NOT NULL AND sender != '' AND sender != 'nova' GROUP BY sender, then LEFT JOINs with contacts table on contact_id to merge relationship_type, notes, channel_ids; returns `Vec<DiscoveredContact>` sorted by last_seen DESC [owner:api-engineer]
- [ ] [1.2] [P-1] Add `discover_relationships()` method to `MessageStore` in `crates/nv-daemon/src/messages.rs` -- executes co-occurrence query: self-join messages table on same channel + same day, group by (sender_a, sender_b, channel), HAVING count >= min_count (default 3); returns `Vec<ContactRelationship>` sorted by co_occurrence_count DESC [owner:api-engineer]
- [ ] [1.3] [P-2] Add SQLite index `idx_messages_sender_direction` on `(sender, direction)` in messages.rs migrations (v14) to accelerate the discovery aggregation query [owner:api-engineer]

## API Batch: HTTP Endpoints

- [ ] [2.1] [P-1] Define Rust structs in `crates/nv-daemon/src/http.rs`: `DiscoveredContact` (name: String, channels: Vec<String>, message_count: i64, first_seen: String, last_seen: String, contact_id: Option<String>, relationship_type: Option<String>, notes: Option<String>, channel_ids: Option<serde_json::Value>), `DiscoveredContactsResponse` (contacts: Vec<DiscoveredContact>, total_senders: i64, total_messages_scanned: i64) [owner:api-engineer]
- [ ] [2.2] [P-1] Define Rust structs in `crates/nv-daemon/src/http.rs`: `ContactRelationship` (person_a: String, person_b: String, shared_channel: String, co_occurrence_count: i64), `RelationshipsResponse` (relationships: Vec<ContactRelationship>), `RelationshipsQuery` (min_count: Option<i64>) [owner:api-engineer]
- [ ] [2.3] [P-1] Add `GET /api/contacts/discovered` handler in `http.rs` -- opens MessageStore, calls `discover_contacts()`, serializes to `DiscoveredContactsResponse`; returns 200 on success, 500 with error JSON on failure [owner:api-engineer]
- [ ] [2.4] [P-1] Add `GET /api/contacts/relationships` handler in `http.rs` -- opens MessageStore, reads `min_count` from query params (default 3), calls `discover_relationships(min_count)`, serializes to `RelationshipsResponse`; returns 200 on success, 500 with error JSON on failure [owner:api-engineer]
- [ ] [2.5] [P-1] Register both new routes in the Axum router in `build_router()` in `http.rs` -- `.route("/api/contacts/discovered", get(discovered_contacts_handler))` and `.route("/api/contacts/relationships", get(relationships_handler))`; placed BEFORE the existing `/api/contacts/{id}` route to avoid path conflicts [owner:api-engineer]

## API Batch: TypeScript Types

- [ ] [3.1] [P-1] Add `DiscoveredContact` interface to `dashboard/src/types/api.ts`: name (string), channels (string[]), message_count (number), first_seen (string), last_seen (string), contact_id (string | null), relationship_type (string | null), notes (string | null), channel_ids (Record<string, string> | null) [owner:api-engineer]
- [ ] [3.2] [P-1] Add `DiscoveredContactsResponse` interface to `dashboard/src/types/api.ts`: contacts (DiscoveredContact[]), total_senders (number), total_messages_scanned (number) [owner:api-engineer]
- [ ] [3.3] [P-1] Add `ContactRelationship` interface to `dashboard/src/types/api.ts`: person_a (string), person_b (string), shared_channel (string), co_occurrence_count (number) [owner:api-engineer]
- [ ] [3.4] [P-1] Add `RelationshipsResponse` interface to `dashboard/src/types/api.ts`: relationships (ContactRelationship[]) [owner:api-engineer]

## UI Batch: Contact Card Component

- [ ] [4.1] [P-1] Create `dashboard/src/components/ContactCard.tsx` -- props: contact (DiscoveredContact), relatedPeople (string[]), onClick (() => void); renders cosmic-surface card with border and hover:border-cosmic-purple/50; shows name (font-medium text-cosmic-bright), channel badges (reuse inline ChannelBadges pattern with capitalize + accent colors), message count (locale-formatted), relative last_seen, relationship pill (reuse RelationshipPill pattern for work/personal-client/contributor/social, add "Untagged" variant with gray styling for null relationship_type), truncated notes (max 80 chars), "Also talks with: X, Y, Z" line (max 3 names, "+N more" suffix) [owner:ui-engineer]

## UI Batch: Contact Detail Panel

- [ ] [5.1] [P-1] Create `dashboard/src/components/ContactDetailPanel.tsx` -- props: contact (DiscoveredContact), relationships (ContactRelationship[]), onClose (() => void); slide-in panel from right with fixed 400px width on desktop, full width on mobile; backdrop overlay with click-to-close; close button (X) top-right [owner:ui-engineer]
- [ ] [5.2] [P-1] Panel content sections: (1) Header: name (text-lg font-semibold text-cosmic-bright), relationship pill, (2) Channels: list each channel with identifier value and copy-to-clipboard button, (3) Activity: message count, first seen date (absolute), last seen date (absolute + relative), (4) Notes: full notes text or "No notes" placeholder, (5) Related People: list of names from relationships data with shared_channel badge and co_occurrence_count; filter relationships where person_a or person_b matches contact.name [owner:ui-engineer]

## UI Batch: Contacts Page Rewrite

- [ ] [6.1] [P-1] Rewrite `dashboard/src/pages/ContactsPage.tsx` -- remove all CRUD imports, types, API helpers (createContact, updateContact, deleteContact, buildChannelIds), remove ContactModal component, remove DeleteConfirm component, remove formFromContact and emptyForm utilities [owner:ui-engineer]
- [ ] [6.2] [P-1] New state shape: contacts (DiscoveredContact[]), relationships (ContactRelationship[]), loading (boolean), error (string | null), search (string), debouncedSearch (string), filterTab (FilterTab extended with "untagged"), selectedContact (DiscoveredContact | null) [owner:ui-engineer]
- [ ] [6.3] [P-1] New data fetching: on mount, fetch `GET /api/contacts/discovered` and `GET /api/contacts/relationships` in parallel using Promise.all; set contacts and relationships state; show skeleton loading (3 card placeholders); show error banner on failure with retry [owner:ui-engineer]
- [ ] [6.4] [P-1] New header section: title "Contacts" with Users icon, subtitle "Discovered N contacts from M conversations" using total_senders and total_messages_scanned from response; Refresh button (existing pattern) [owner:ui-engineer]
- [ ] [6.5] [P-1] Search input: reuse existing search UI pattern with debounce (300ms); client-side filter on contacts array by name.toLowerCase().includes(debouncedSearch.toLowerCase()) [owner:ui-engineer]
- [ ] [6.6] [P-1] Filter tabs: extend FILTER_TABS with { key: "untagged", label: "Untagged" }; filter logic: "all" shows everything, "untagged" shows contacts where relationship_type is null, others filter by exact relationship_type match [owner:ui-engineer]
- [ ] [6.7] [P-1] Contact grid: responsive grid layout (grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4); render ContactCard for each filtered contact; compute relatedPeople for each card by filtering relationships where person_a or person_b matches contact.name, extracting the other person's name, deduplicating [owner:ui-engineer]
- [ ] [6.8] [P-1] Detail panel integration: when selectedContact is not null, render ContactDetailPanel with the selected contact, full relationships array, and onClose that sets selectedContact to null [owner:ui-engineer]
- [ ] [6.9] [P-1] Inline `relativeTime(iso: string): string` utility function -- converts ISO timestamp to human-readable relative time: <60s = "just now", <60m = "Nm ago", <24h = "Nh ago", <7d = "Nd ago", <30d = "Nw ago", <365d = "Nmo ago", else ">1y ago" [owner:ui-engineer]
- [ ] [6.10] [P-2] Empty state: when contacts array is empty after loading, show "No conversations yet" message with Users icon and text "Nova hasn't received any messages yet. Contacts will appear automatically as people message Nova across channels." [owner:ui-engineer]

## E2E Batch: Verification

- [ ] [7.1] Rust compilation: `cargo check -p nv-daemon` passes with no errors [owner:api-engineer]
- [ ] [7.2] TypeScript compilation: dashboard `pnpm --filter dashboard typecheck` passes with no errors (or `npx tsc --noEmit` in dashboard/) [owner:ui-engineer]
- [ ] [7.3] [user] Manual smoke: start daemon, navigate to /contacts, verify discovered contacts load from message history sorted by last interaction
- [ ] [7.4] [user] Manual smoke: verify contact cards show channel badges, message count, relative last seen time, and relationship pill
- [ ] [7.5] [user] Manual smoke: click a contact card, verify detail panel slides in from right with full profile info and related people
- [ ] [7.6] [user] Manual smoke: verify search filters contacts by name in real-time
- [ ] [7.7] [user] Manual smoke: verify filter tabs (All, Work, Contributor, Social, Untagged) correctly filter the grid
- [ ] [7.8] [user] Manual smoke: verify "New Contact" button and edit/delete actions are gone
- [ ] [7.9] [user] Manual smoke: verify subtitle shows correct "Discovered N contacts from M conversations" counts
