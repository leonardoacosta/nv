pub mod followup;

use std::sync::{Arc, Mutex};

use crate::obligation_store::ObligationStore;
use nv_core::types::ObligationStatus;

/// Build a research-notes context block for all open obligations that have
/// research results.
///
/// Returns `None` if there are no open obligations with research, or if the
/// obligation store is unavailable. Caller wraps the result in
/// `<obligation_research_context>` tags before injecting into the user message.
#[allow(dead_code)] // wired by proactive-obligation-research spec
pub fn obligation_research_context(
    obligation_store: &Option<Arc<Mutex<ObligationStore>>>,
) -> Option<String> {
    let store_arc = obligation_store.as_ref()?;
    let store = store_arc.lock().ok()?;

    let open_obligations = store
        .list_by_status(&ObligationStatus::Open)
        .unwrap_or_default();

    if open_obligations.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    for ob in &open_obligations {
        match store.get_latest_research(&ob.id) {
            Ok(Some(notes)) => {
                let mut entry = format!(
                    "Obligation [{}] (priority {}): {}\nResearch: {}",
                    ob.id, ob.priority, ob.detected_action, notes.summary
                );
                for f in &notes.raw_findings {
                    entry.push_str(&format!("\n  - [{}] {}", f.tool, f.label));
                }
                parts.push(entry);
            }
            Ok(None) => {} // no research yet — skip
            Err(e) => {
                tracing::debug!(
                    obligation_id = %ob.id,
                    error = %e,
                    "failed to load research for obligation context"
                );
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}
