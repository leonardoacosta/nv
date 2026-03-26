# Implementation Tasks

<!-- beads:epic:nv-7bd5 -->

## DB Batch

- [x] [1.1] [P-1] Add `contacts` table migration to `messages_migrations()` in `messages.rs` — UUID PK, name, channel_ids JSON, relationship_type CHECK enum, notes, created_at, updated_at; indexes on name and relationship_type [owner:db-engineer]
- [x] [1.2] [P-1] Add `messages.contact_id` migration — `ALTER TABLE messages ADD COLUMN contact_id TEXT REFERENCES contacts(id)`, index on contact_id, best-effort backfill UPDATE using `json_extract` [owner:db-engineer]
- [x] [1.3] [P-1] Update `MessageStore::log_inbound` signature to accept `contact_id: Option<&str>` and include it in the INSERT [owner:db-engineer]

## API Batch

- [x] [2.1] [P-1] Create `crates/nv-daemon/src/contact_store.rs` — `Contact` struct (id, name, channel_ids as `serde_json::Value`, relationship_type, notes, created_at, updated_at) [owner:api-engineer]
- [x] [2.2] [P-1] Implement `ContactStore::new(conn: Arc<Mutex<Connection>>)`, `create`, `get`, `update`, `delete` methods [owner:api-engineer]
- [x] [2.3] [P-1] Implement `ContactStore::list(relationship_type: Option<&str>)` and `search(query: &str)` (LIKE on name and notes) [owner:api-engineer]
- [x] [2.4] [P-1] Implement `ContactStore::find_by_channel(channel: &str, identifier: &str)` using `json_extract(channel_ids, '$.<channel>')` [owner:api-engineer]
- [x] [2.5] [P-1] Add `mod contact_store;` in `main.rs`; init `ContactStore` on startup and store in `AppState` and `HttpState` [owner:api-engineer]
- [x] [2.6] [P-1] Add `ContactsConfig` struct to `nv-core/src/config.rs` (`profile_dir`, `inject_in_context`); add `contacts: Option<ContactsConfig>` to `Config` [owner:api-engineer]
- [x] [2.7] [P-1] Register 5 HTTP routes in `http.rs`: GET/POST `/api/contacts`, GET/PUT/DELETE `/api/contacts/{id}`; implement all 5 handlers returning `Json<Contact>` or `StatusCode` [owner:api-engineer]
- [x] [2.8] [P-2] Wire `find_by_channel("telegram", ...)` call in `channels/telegram/mod.rs` ingest path; pass resolved `contact_id` to `log_inbound` [owner:api-engineer]
- [x] [2.9] [P-2] Wire `find_by_channel("discord", ...)` in `channels/discord/mod.rs` + `gateway.rs`; pass `contact_id` to `log_inbound` [owner:api-engineer]
- [x] [2.10] [P-2] Wire `find_by_channel("teams", ...)` in `channels/teams/mod.rs`; pass `contact_id` to `log_inbound` [owner:api-engineer]
- [x] [2.11] [P-2] Update all remaining `log_inbound` call sites (non-channel paths) to pass `None` for `contact_id` [owner:api-engineer]
- [x] [2.12] [P-2] Implement contact profile context injection — read `config/contact/*.md` files (skip `example-*` prefix); inject under `## Contacts` in system prompt when `inject_in_context = true`; cap at 4KB with truncation warning [owner:api-engineer]
- [x] [2.13] [P-2] Create `config/contact/.gitkeep` and `config/contact/example-contact.md` documenting the profile file format [owner:api-engineer]

## UI Batch

- [x] [3.1] [P-1] Create `dashboard/src/pages/ContactsPage.tsx` — fetches GET `/api/contacts`, renders contact list with Name, Relationship pill, Channels (text labels), Notes (truncated 60 chars), Edit/Delete actions [owner:ui-engineer]
- [x] [3.2] [P-1] Add search bar (debounced, 300ms) and relationship filter tabs (`all | work | personal-client | contributor | social`) to ContactsPage — query params drive API fetch [owner:ui-engineer]
- [x] [3.3] [P-1] Implement create/edit modal in ContactsPage — fields: Name (required), Relationship dropdown, Telegram identifier, Discord identifier, Teams UPN, Notes textarea; POST on create, PUT on edit [owner:ui-engineer]
- [x] [3.4] [P-1] Implement delete confirmation in ContactsPage — inline confirm state before calling DELETE `/api/contacts/{id}` [owner:ui-engineer]
- [x] [3.5] [P-1] Add "Contacts" entry to `dashboard/src/components/Sidebar.tsx` [owner:ui-engineer]
- [x] [3.6] [P-1] Wire `/contacts` route in `dashboard/src/App.tsx` [owner:ui-engineer]

## Verify

- [x] [4.1] `cargo build` passes [owner:api-engineer]
- [x] [4.2] `cargo clippy -- -D warnings` passes [owner:api-engineer]
- [x] [4.3] Unit test: `ContactStore::create` roundtrips — create then `get` returns same data [owner:api-engineer]
- [x] [4.4] Unit test: `ContactStore::find_by_channel` returns correct contact for matching identifier [owner:api-engineer]
- [x] [4.5] Unit test: `ContactStore::find_by_channel` returns `None` for unknown identifier [owner:api-engineer]
- [x] [4.6] Unit test: `ContactStore::search` matches on name substring [owner:api-engineer]
- [x] [4.7] Unit test: `ContactStore::list` with `relationship_type = Some("work")` returns only work contacts [owner:api-engineer]
- [x] [4.8] Unit test: `log_inbound` with `contact_id = Some("uuid")` stores the FK correctly (query messages table) [owner:api-engineer]
- [x] [4.9] Unit test: contact profile context injection skips `example-*.md` files and caps output at 4KB [owner:api-engineer]
- [x] [4.10] Existing tests pass (0 regressions) [owner:api-engineer]
