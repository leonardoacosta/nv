# Proposal: Contact Profiles System

## Change ID
`contact-profiles-system`

## Summary

Full contact profiles system. SQLite `contacts` table with CRUD + search via `ContactStore`.
Sender FK migration adds `contact_id` to the `messages` table and backfills existing senders by
name/channel matching. Markdown profile files in `config/contact/` give Nova stable, long-term
details to reference in system context. Dashboard contacts page with list, search, and
relationship filter.

## Context
- Extends: `crates/nv-daemon/src/messages.rs` (new migration), `crates/nv-daemon/src/http.rs`
  (contact API routes), `crates/nv-core/src/config.rs` (optional `contacts` section),
  `dashboard/src/` (new contacts page + routing)
- Channel files affected: `channels/telegram/mod.rs`, `channels/discord/mod.rs`,
  `channels/discord/gateway.rs`, `channels/teams/mod.rs` (contact_id lookup on ingest)
- Depends on: none (messages table already exists; `contact_id` added via migration)
- Related: PRD §3 — Data & Integrations | nv-0bxt

## Motivation

Nova interacts with a stable, small set of real people across Telegram, Discord, and Teams. Today
every sender is an opaque string — "Leo", "@lacosta", a Teams UPN. Nova has no structured model of
who these people are, how to weight their messages, or what context governs replies (work hours vs
personal availability).

A contact profile system solves three things:

1. **Identity consolidation** — one `Contact` row links a person's Telegram handle, Discord user
   ID, and Teams UPN. When "Leo" messages from any channel Nova knows it's the same person.
2. **Relationship context** — `relationship_type` drives context bifurcation. `work` contacts
   respect the 9–5 boundary; `personal-client`, `contributor`, and `social` contacts get the
   personal Nova persona and relaxed scheduling rules.
3. **Stable reference** — `config/contact/<slug>.md` files hold long-form notes (timezone,
   preferred style, ongoing projects) that survive database wipes and are committed to git. Nova
   can include these files in system context without hitting the database.

## Requirements

### Req-1: SQLite `contacts` Table (migration)

Add a new versioned migration to `messages_migrations()` in `messages.rs`:

```sql
CREATE TABLE IF NOT EXISTS contacts (
    id          TEXT PRIMARY KEY,          -- UUID v4
    name        TEXT NOT NULL,             -- display name ("Leo Acosta")
    channel_ids TEXT NOT NULL DEFAULT '{}', -- JSON: {"telegram":"@lacosta","discord":"123","teams":"upn@..."}
    relationship_type TEXT NOT NULL DEFAULT 'social'
                CHECK(relationship_type IN ('work','personal-client','contributor','social')),
    notes       TEXT,                      -- free-form operator notes
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts(name);
CREATE INDEX IF NOT EXISTS idx_contacts_relationship ON contacts(relationship_type);
```

### Req-2: `contacts.channel_ids` FK Column on `messages`

Second migration adds `contact_id` to `messages` and runs a one-time backfill:

```sql
ALTER TABLE messages ADD COLUMN contact_id TEXT REFERENCES contacts(id);
CREATE INDEX IF NOT EXISTS idx_messages_contact_id ON messages(contact_id);
```

Backfill strategy: for each distinct `(sender, channel)` pair in `messages`, attempt a lookup in
`contacts.channel_ids` JSON. If the sender string appears as a value under the matching channel
key, set `contact_id`. This is a best-effort single-pass UPDATE — no rows are deleted if no match
is found.

### Req-3: `ContactStore` — CRUD + Search

Create `crates/nv-daemon/src/contact_store.rs`. The store shares a `Connection` (or `Arc<Mutex<Connection>>`) with `MessageStore`. Public API:

```rust
pub struct Contact {
    pub id: String,
    pub name: String,
    pub channel_ids: serde_json::Value,  // {"telegram":"...","discord":"...","teams":"..."}
    pub relationship_type: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ContactStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self;

    // CRUD
    pub fn create(&self, name: &str, channel_ids: serde_json::Value,
                  relationship_type: &str, notes: Option<&str>) -> Result<Contact>;
    pub fn get(&self, id: &str) -> Result<Option<Contact>>;
    pub fn update(&self, id: &str, name: Option<&str>, channel_ids: Option<serde_json::Value>,
                  relationship_type: Option<&str>, notes: Option<&str>) -> Result<Contact>;
    pub fn delete(&self, id: &str) -> Result<bool>;

    // Query
    pub fn list(&self, relationship_type: Option<&str>) -> Result<Vec<Contact>>;
    pub fn search(&self, query: &str) -> Result<Vec<Contact>>;  // LIKE on name + notes
    pub fn find_by_channel(&self, channel: &str, identifier: &str) -> Result<Option<Contact>>;
}
```

`find_by_channel(channel, identifier)` is the hot path used during message ingestion: performs a
`json_extract(channel_ids, '$.<channel>')` lookup.

### Req-4: Sender Enrichment in Channel Ingestion

After `ContactStore` is available in `AppState`, the ingest path for each channel looks up the
contact on every inbound message and attaches `contact_id`:

- **Telegram** (`channels/telegram/mod.rs`): use `msg.sender` (Telegram username or first name)
  with `find_by_channel("telegram", sender)`.
- **Discord** (`channels/discord/mod.rs` + `gateway.rs`): use Discord user ID string with
  `find_by_channel("discord", user_id)`.
- **Teams** (`channels/teams/mod.rs`): use UPN from the Graph API payload with
  `find_by_channel("teams", upn)`.

The resolved `contact_id` (or `None`) is passed to `MessageStore::log_inbound` via a new optional
field added to the log call. No ingest failure if lookup returns `None` — contacts are opt-in.

### Req-5: `MessageStore::log_inbound` signature update

Add `contact_id: Option<&str>` parameter to `log_inbound`. All existing call sites pass `None`
until Req-4 is wired. The INSERT stores the value in the new column.

### Req-6: HTTP API (contact CRUD + list)

Register routes in `http.rs`:

| Method | Route | Handler |
|--------|-------|---------|
| GET | `/api/contacts` | list — accepts `?relationship=work` filter and `?q=` search |
| POST | `/api/contacts` | create |
| GET | `/api/contacts/{id}` | get single |
| PUT | `/api/contacts/{id}` | update |
| DELETE | `/api/contacts/{id}` | delete |

Response shape (JSON):

```json
{
  "id": "uuid",
  "name": "Leo Acosta",
  "channel_ids": {"telegram": "@lacosta", "discord": "123456"},
  "relationship_type": "work",
  "notes": null,
  "created_at": "2026-03-25T00:00:00Z",
  "updated_at": "2026-03-25T00:00:00Z"
}
```

### Req-7: `config/contact/*.md` Profile Files

Introduce the `config/contact/` directory. Each file is `<slug>.md` (e.g., `leo-acosta.md`).
Format is free-form Markdown — operator-maintained. Nova's system context builder
(`orchestrator.rs` or `memory.rs`) reads all files in this directory and injects them under a
`## Contacts` heading in the system prompt when the directory is non-empty.

No structured front matter is required. Operators write whatever is useful: timezone, preferred
communication style, ongoing projects, do-not-disturb windows. The file is the authoritative
long-term record; the DB row is the runtime identity anchor.

Add a `.gitkeep` to create the directory; add `config/contact/example-contact.md` as a documented
template (excluded from Nova context injection — checked by filename prefix `example-`).

### Req-8: `nv.toml` Config Section (optional)

Add optional `[contacts]` section to `Config` in `nv-core/src/config.rs`:

```toml
[contacts]
profile_dir = "config/contact"   # default; resolved relative to config root
inject_in_context = true          # default true
```

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ContactsConfig {
    #[serde(default = "default_contact_profile_dir")]
    pub profile_dir: String,
    #[serde(default = "default_contact_inject")]
    pub inject_in_context: bool,
}
```

When absent, defaults apply. `Config.contacts` field is `Option<ContactsConfig>`.

### Req-9: Dashboard Contacts Page

Add `dashboard/src/pages/ContactsPage.tsx`:

- **List view**: table/card list of all contacts, columns: Name, Relationship, Channels (icons),
  Notes (truncated), Actions (Edit / Delete).
- **Search bar**: debounced `?q=` query param → live filter via GET `/api/contacts?q=`.
- **Relationship filter**: tab or pill filter for `all | work | personal-client | contributor | social`.
- **Create / Edit modal**: form with Name, Relationship dropdown, per-channel identifier fields
  (Telegram, Discord, Teams), Notes textarea.
- **Delete confirmation**: inline confirm before DELETE.

Wire `ContactsPage` into `App.tsx` routing and add "Contacts" entry to `Sidebar.tsx`.

## Scope

- **IN**: `contacts` table, `ContactStore`, messages `contact_id` migration + backfill, channel
  ingestion enrichment, HTTP CRUD API, `config/contact/` profile files, system context injection,
  dashboard page + sidebar entry.
- **OUT**: Contact deduplication AI (merging duplicates automatically), cross-contact analytics
  ("who messages most"), contact-based routing rules, contact import/export (CSV/vCard), notification
  preferences per contact, relationship-type enforcement at the worker level (that is a separate
  context-bifurcation spec).

## Impact

| Area | Change |
|------|--------|
| `crates/nv-daemon/src/contact_store.rs` | New module: `Contact` struct, `ContactStore` with full CRUD + search + `find_by_channel` |
| `crates/nv-daemon/src/messages.rs` | Two new migrations: `contacts` table, `messages.contact_id` column + backfill; `log_inbound` gains `contact_id: Option<&str>` |
| `crates/nv-daemon/src/http.rs` | 5 new routes under `/api/contacts`; `HttpState` gains `Arc<ContactStore>` |
| `crates/nv-daemon/src/main.rs` | `mod contact_store;` declaration; init `ContactStore` and pass to `HttpState` |
| `crates/nv-daemon/src/state.rs` | `AppState` gains `contact_store: Arc<ContactStore>` |
| `crates/nv-daemon/src/channels/telegram/mod.rs` | `find_by_channel` lookup, pass `contact_id` to `log_inbound` |
| `crates/nv-daemon/src/channels/discord/mod.rs` + `gateway.rs` | Same: Discord user ID lookup |
| `crates/nv-daemon/src/channels/teams/mod.rs` | Same: Teams UPN lookup |
| `crates/nv-core/src/config.rs` | `ContactsConfig` struct, `Config.contacts: Option<ContactsConfig>` |
| `config/contact/` | New directory: `.gitkeep` + `example-contact.md` template |
| `dashboard/src/pages/ContactsPage.tsx` | New page: list + search + filter + create/edit/delete modal |
| `dashboard/src/components/Sidebar.tsx` | Add Contacts nav entry |
| `dashboard/src/App.tsx` | Wire contacts route |

## Risks

| Risk | Mitigation |
|------|-----------|
| `contact_id` backfill mismatches (sender string format varies by channel) | Backfill is best-effort UPDATE; no rows deleted on miss; operator can manually fix via dashboard |
| `log_inbound` signature change breaks all call sites | All call sites updated in same PR; compiler enforces exhaustiveness |
| Profile files injected into context exceed token budget | Inject only when `inject_in_context = true`; cap total injected content at 4KB with truncation warning |
| Dashboard page grows complex with modals | Keep modal as a single controlled component; no external modal library needed |
| `ContactStore` and `MessageStore` share the same SQLite file | Both already use the same `~/.nv/messages.db`; share the same `Connection` under `Arc<Mutex<>>` — no second DB file needed |
