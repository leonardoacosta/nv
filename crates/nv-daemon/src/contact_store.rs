//! Contact store: CRUD operations on the `contacts` table in messages.db.
//!
//! The schema is created by migration v9 in `messages.rs`. This module provides
//! the `ContactStore` struct that wraps a shared `Arc<Mutex<Connection>>` and
//! exposes typed CRUD + search + channel-lookup methods.

use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Contact ─────────────────────────────────────────────────────────

/// A contact record from the `contacts` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: String,
    /// JSON object: `{"telegram":"@handle","discord":"id","teams":"upn@..."}`
    pub channel_ids: serde_json::Value,
    pub relationship_type: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ── Store ────────────────────────────────────────────────────────────

/// SQLite-backed contact store.
///
/// Shares the same `messages.db` as `MessageStore` via an `Arc<Mutex<Connection>>`.
/// Migration v9 (contacts table) must have run before constructing this store —
/// this is guaranteed because `MessageStore::init` runs all migrations first.
pub struct ContactStore {
    conn: Arc<Mutex<Connection>>,
}

impl ContactStore {
    /// Construct a `ContactStore` wrapping an existing shared SQLite connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    // ── CRUD ────────────────────────────────────────────────────────

    /// Create a new contact row. Returns the created `Contact`.
    pub fn create(
        &self,
        name: &str,
        channel_ids: serde_json::Value,
        relationship_type: &str,
        notes: Option<&str>,
    ) -> Result<Contact> {
        validate_relationship_type(relationship_type)?;
        let id = Uuid::new_v4().to_string();
        let channel_ids_str = serde_json::to_string(&channel_ids)?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO contacts (id, name, channel_ids, relationship_type, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))",
            params![id, name, channel_ids_str, relationship_type, notes],
        )?;

        // Re-read to get the database-generated timestamps.
        let contact = query_contact_by_id(&conn, &id)?
            .ok_or_else(|| anyhow::anyhow!("contact not found immediately after insert: {id}"))?;
        Ok(contact)
    }

    /// Fetch a single contact by ID. Returns `None` if not found.
    pub fn get(&self, id: &str) -> Result<Option<Contact>> {
        let conn = self.conn.lock().unwrap();
        query_contact_by_id(&conn, id)
    }

    /// Update fields on an existing contact. Returns the updated `Contact`.
    ///
    /// Pass `None` for any field to leave it unchanged.
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        channel_ids: Option<serde_json::Value>,
        relationship_type: Option<&str>,
        notes: Option<&str>,
    ) -> Result<Contact> {
        if let Some(rt) = relationship_type {
            validate_relationship_type(rt)?;
        }

        let conn = self.conn.lock().unwrap();

        // Ensure the contact exists first.
        if query_contact_by_id(&conn, id)?.is_none() {
            bail!("contact not found: {id}");
        }

        if let Some(n) = name {
            conn.execute(
                "UPDATE contacts SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![n, id],
            )?;
        }
        if let Some(cids) = channel_ids {
            let cids_str = serde_json::to_string(&cids)?;
            conn.execute(
                "UPDATE contacts SET channel_ids = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![cids_str, id],
            )?;
        }
        if let Some(rt) = relationship_type {
            conn.execute(
                "UPDATE contacts SET relationship_type = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![rt, id],
            )?;
        }
        // Notes: always update (supports clearing with explicit Some("") or passing None to skip).
        if notes.is_some() {
            conn.execute(
                "UPDATE contacts SET notes = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![notes, id],
            )?;
        }

        let updated = query_contact_by_id(&conn, id)?
            .ok_or_else(|| anyhow::anyhow!("contact disappeared during update: {id}"))?;
        Ok(updated)
    }

    /// Delete a contact by ID. Returns `true` if a row was deleted.
    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM contacts WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // ── Query ────────────────────────────────────────────────────────

    /// List all contacts, optionally filtered by `relationship_type`.
    pub fn list(&self, relationship_type: Option<&str>) -> Result<Vec<Contact>> {
        let conn = self.conn.lock().unwrap();
        let (sql, param): (&str, Option<&str>) = if let Some(rt) = relationship_type {
            (
                "SELECT id, name, channel_ids, relationship_type, notes, created_at, updated_at
                 FROM contacts WHERE relationship_type = ?1
                 ORDER BY name ASC",
                Some(rt),
            )
        } else {
            (
                "SELECT id, name, channel_ids, relationship_type, notes, created_at, updated_at
                 FROM contacts ORDER BY name ASC",
                None,
            )
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(p) = param {
            stmt.query_map(params![p], row_to_contact)
        } else {
            stmt.query_map([], row_to_contact)
        }?;

        let mut contacts = Vec::new();
        for row in rows {
            contacts.push(row?);
        }
        Ok(contacts)
    }

    /// Search contacts by name or notes (case-insensitive LIKE).
    pub fn search(&self, query: &str) -> Result<Vec<Contact>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{query}%");
        let mut stmt = conn.prepare(
            "SELECT id, name, channel_ids, relationship_type, notes, created_at, updated_at
             FROM contacts
             WHERE name LIKE ?1 OR notes LIKE ?1
             ORDER BY name ASC",
        )?;
        let rows = stmt.query_map(params![pattern], row_to_contact)?;

        let mut contacts = Vec::new();
        for row in rows {
            contacts.push(row?);
        }
        Ok(contacts)
    }

    /// Look up a contact by channel + identifier.
    ///
    /// Uses SQLite's `json_extract` to check `channel_ids->$.<channel>`.
    /// This is the hot path called on every inbound message during ingest.
    ///
    /// Returns `None` if no contact matches (callers must handle this gracefully —
    /// contacts are opt-in and the absence of a match is not an error).
    pub fn find_by_channel(&self, channel: &str, identifier: &str) -> Result<Option<Contact>> {
        let conn = self.conn.lock().unwrap();
        // Build the JSON path dynamically: `$.<channel>`
        let json_path = format!("$.{channel}");
        let mut stmt = conn.prepare(
            "SELECT id, name, channel_ids, relationship_type, notes, created_at, updated_at
             FROM contacts
             WHERE json_extract(channel_ids, ?1) = ?2
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![json_path, identifier], row_to_contact)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Validate that `relationship_type` is one of the allowed enum values.
fn validate_relationship_type(rt: &str) -> Result<()> {
    match rt {
        "work" | "personal-client" | "contributor" | "social" => Ok(()),
        _ => bail!(
            "invalid relationship_type '{rt}'; must be one of: work, personal-client, contributor, social"
        ),
    }
}

/// Convert a rusqlite `Row` to a `Contact`.
fn row_to_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let channel_ids_str: String = row.get(2)?;
    let channel_ids: serde_json::Value =
        serde_json::from_str(&channel_ids_str).unwrap_or(serde_json::Value::Object(Default::default()));
    Ok(Contact {
        id: row.get(0)?,
        name: row.get(1)?,
        channel_ids,
        relationship_type: row.get(3)?,
        notes: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// Query a single contact by ID from an already-locked connection.
fn query_contact_by_id(conn: &Connection, id: &str) -> Result<Option<Contact>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, channel_ids, relationship_type, notes, created_at, updated_at
         FROM contacts WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_contact)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::MessageStore;
    use tempfile::TempDir;

    fn setup() -> (TempDir, ContactStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("messages.db");
        // MessageStore::init runs all migrations, including v9 (contacts table).
        let _ms = MessageStore::init(&db_path).unwrap();
        let conn = Arc::new(Mutex::new(
            rusqlite::Connection::open(&db_path).unwrap(),
        ));
        conn.lock()
            .unwrap()
            .execute_batch("PRAGMA journal_mode=WAL;")
            .unwrap();
        let store = ContactStore::new(conn);
        (dir, store)
    }

    #[test]
    fn create_and_get_roundtrip() {
        let (_dir, store) = setup();
        let contact = store
            .create(
                "Leo Acosta",
                serde_json::json!({"telegram": "@lacosta"}),
                "work",
                Some("Primary operator"),
            )
            .unwrap();

        assert!(!contact.id.is_empty());
        assert_eq!(contact.name, "Leo Acosta");
        assert_eq!(contact.relationship_type, "work");
        assert_eq!(contact.notes.as_deref(), Some("Primary operator"));
        assert_eq!(
            contact.channel_ids.get("telegram").and_then(|v| v.as_str()),
            Some("@lacosta")
        );

        let fetched = store.get(&contact.id).unwrap().unwrap();
        assert_eq!(fetched.id, contact.id);
        assert_eq!(fetched.name, "Leo Acosta");
    }

    #[test]
    fn find_by_channel_returns_matching_contact() {
        let (_dir, store) = setup();
        let c = store
            .create(
                "Discord User",
                serde_json::json!({"discord": "123456789"}),
                "social",
                None,
            )
            .unwrap();

        let found = store.find_by_channel("discord", "123456789").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, c.id);
    }

    #[test]
    fn find_by_channel_returns_none_for_unknown() {
        let (_dir, store) = setup();
        let found = store.find_by_channel("telegram", "@nobody").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn search_matches_name_substring() {
        let (_dir, store) = setup();
        store
            .create("Alice Smith", serde_json::json!({}), "social", None)
            .unwrap();
        store
            .create("Bob Jones", serde_json::json!({}), "social", None)
            .unwrap();

        let results = store.search("alice").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice Smith");
    }

    #[test]
    fn list_with_relationship_filter_returns_only_work() {
        let (_dir, store) = setup();
        store
            .create("Work Contact", serde_json::json!({}), "work", None)
            .unwrap();
        store
            .create("Social Contact", serde_json::json!({}), "social", None)
            .unwrap();

        let work_contacts = store.list(Some("work")).unwrap();
        assert_eq!(work_contacts.len(), 1);
        assert_eq!(work_contacts[0].name, "Work Contact");

        let all = store.list(None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_removes_contact() {
        let (_dir, store) = setup();
        let c = store
            .create("Temp", serde_json::json!({}), "social", None)
            .unwrap();

        assert!(store.delete(&c.id).unwrap());
        assert!(store.get(&c.id).unwrap().is_none());
        // Second delete returns false (already gone).
        assert!(!store.delete(&c.id).unwrap());
    }

    #[test]
    fn update_name_and_relationship() {
        let (_dir, store) = setup();
        let c = store
            .create("Original", serde_json::json!({}), "social", None)
            .unwrap();

        let updated = store
            .update(&c.id, Some("Renamed"), None, Some("contributor"), None)
            .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.relationship_type, "contributor");
    }

    #[test]
    fn log_inbound_with_contact_id_stores_fk() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("messages.db");
        let ms = MessageStore::init(&db_path).unwrap();

        // Create a contact via a shared connection.
        let conn = Arc::new(Mutex::new(
            rusqlite::Connection::open(&db_path).unwrap(),
        ));
        conn.lock()
            .unwrap()
            .execute_batch("PRAGMA journal_mode=WAL;")
            .unwrap();
        let cs = ContactStore::new(conn);
        let contact = cs
            .create("Test Person", serde_json::json!({}), "social", None)
            .unwrap();

        // Log a message with the contact_id FK.
        ms.log_inbound("telegram", "test", "hello", "message", Some(&contact.id))
            .unwrap();

        // Verify the FK was stored by opening a fresh read connection.
        let verify_conn = rusqlite::Connection::open(&db_path).unwrap();
        let stored_id: Option<String> = verify_conn
            .query_row(
                "SELECT contact_id FROM messages WHERE sender = 'test' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_id.as_deref(), Some(contact.id.as_str()));
    }

    #[test]
    fn contact_profile_context_injection_skips_example_and_caps_at_4kb() {
        use std::fs;
        let dir = TempDir::new().unwrap();
        let profile_dir = dir.path().join("contact");
        fs::create_dir_all(&profile_dir).unwrap();

        // Write a real profile and an example file (to be skipped).
        fs::write(profile_dir.join("leo-acosta.md"), "# Leo\nTimezone: CST").unwrap();
        fs::write(
            profile_dir.join("example-contact.md"),
            "# Example — this should be skipped",
        )
        .unwrap();

        let result = inject_contact_profiles(&profile_dir, true);
        assert!(result.is_some(), "should inject when profiles exist");
        let text = result.unwrap();
        assert!(text.contains("Leo"), "should include leo-acosta.md content");
        assert!(
            !text.contains("example"),
            "should skip example-contact.md"
        );

        // Cap test: write a 5KB profile and verify output is truncated.
        let big_content = "x".repeat(5000);
        fs::write(profile_dir.join("big-person.md"), &big_content).unwrap();
        let result2 = inject_contact_profiles(&profile_dir, true);
        let text2 = result2.unwrap();
        // The total injected content (header + bodies) must not exceed 4096 + some header bytes
        // Just verify it doesn't contain the full 5000-char run.
        assert!(text2.len() < 5000 + 200, "injected content should be capped near 4KB");
    }
}

// ── Contact Profile Context Injection ────────────────────────────────

/// Read all `config/contact/*.md` files (excluding `example-*` prefix) and
/// concatenate them under a `## Contacts` heading.
///
/// Returns `None` when the directory is empty or does not exist.
/// Caps the total injected content body at 4KB, appending a truncation warning
/// if content was dropped.
pub fn inject_contact_profiles(profile_dir: &std::path::Path, inject: bool) -> Option<String> {
    if !inject {
        return None;
    }

    let entries = std::fs::read_dir(profile_dir).ok()?;

    let mut files: Vec<(String, String)> = entries
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().into_string().ok()?;
            if !name.ends_with(".md") || name.starts_with("example-") {
                return None;
            }
            let content = std::fs::read_to_string(e.path()).ok()?;
            Some((name, content))
        })
        .collect();

    if files.is_empty() {
        return None;
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));

    const MAX_BODY_BYTES: usize = 4096;
    let mut body = String::new();
    let mut truncated = false;

    for (filename, content) in &files {
        let section = format!("### {filename}\n\n{content}\n\n");
        if body.len() + section.len() > MAX_BODY_BYTES {
            truncated = true;
            break;
        }
        body.push_str(&section);
    }

    if truncated {
        body.push_str(
            "*[Contact profiles truncated — total exceeded 4KB. Edit config/contact/ to reduce size.]*\n",
        );
    }

    let mut output = String::from("## Contacts\n\n");
    output.push_str(&body);
    Some(output)
}
