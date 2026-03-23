pub mod client;
pub mod html;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use nv_core::channel::Channel;
use nv_core::config::EmailConfig;
use nv_core::types::{InboundMessage, OutboundMessage, Trigger};
use tokio::sync::{mpsc, Mutex};

use self::client::EmailClient;
use self::html::html_to_text;

/// Email channel adapter via MS Graph API.
///
/// Implements the `Channel` trait from nv-core. Polls configured mail
/// folders on a configurable interval, filters by sender and subject,
/// converts HTML bodies to plain text, and sends replies via MS Graph.
pub struct EmailChannel {
    client: EmailClient,
    config: EmailConfig,
    trigger_tx: mpsc::Sender<Trigger>,
    /// Last-seen receivedDateTime per folder (ISO 8601).
    last_seen: Arc<Mutex<HashMap<String, String>>>,
}

impl EmailChannel {
    /// Create a new Email channel.
    ///
    /// - `client`: MS Graph email REST client (with shared auth).
    /// - `config`: Email configuration from `nv.toml`.
    /// - `trigger_tx`: Sender for the daemon's trigger channel.
    pub fn new(
        client: EmailClient,
        config: EmailConfig,
        trigger_tx: mpsc::Sender<Trigger>,
    ) -> Self {
        Self {
            client,
            config,
            trigger_tx,
            last_seen: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a message sender matches the configured sender filter.
    ///
    /// Returns true if:
    /// - The filter is empty (allow all)
    /// - The sender address matches an entry exactly
    /// - The sender address domain matches a domain entry (e.g., "@example.com")
    fn matches_sender_filter(&self, sender_address: &str) -> bool {
        if self.config.sender_filter.is_empty() {
            return true;
        }

        let sender_lower = sender_address.to_ascii_lowercase();

        self.config.sender_filter.iter().any(|filter| {
            let filter_lower = filter.to_ascii_lowercase();
            if filter_lower.starts_with('@') {
                // Domain match
                sender_lower.ends_with(&filter_lower)
            } else if filter_lower.contains('@') {
                // Exact email match
                sender_lower == filter_lower
            } else {
                // Treat as domain without @
                sender_lower.ends_with(&format!("@{filter_lower}"))
            }
        })
    }

    /// Check if a message subject matches the configured subject filter.
    ///
    /// Returns true if:
    /// - The filter is empty (allow all)
    /// - The subject contains any of the filter substrings (case-insensitive)
    fn matches_subject_filter(&self, subject: &str) -> bool {
        if self.config.subject_filter.is_empty() {
            return true;
        }

        let subject_lower = subject.to_ascii_lowercase();
        self.config
            .subject_filter
            .iter()
            .any(|filter| subject_lower.contains(&filter.to_ascii_lowercase()))
    }

    /// Extract plain text from a mail message body.
    fn extract_body_text(msg: &types::GraphMailMessage) -> String {
        if let Some(body) = &msg.body {
            let is_html = body
                .content_type
                .as_deref()
                .map(|ct| ct.eq_ignore_ascii_case("html"))
                .unwrap_or(false);

            if is_html {
                html_to_text(&body.content)
            } else {
                body.content.clone()
            }
        } else {
            // Fall back to body preview if no full body
            msg.body_preview.clone().unwrap_or_default()
        }
    }
}

#[async_trait]
impl Channel for EmailChannel {
    fn name(&self) -> &str {
        "email"
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        // Authenticate (reuses shared MsGraphAuth)
        self.client.auth().authenticate().await?;
        tracing::info!("Email channel OAuth authenticated");

        // Initialize last_seen to "now" so we only pick up new messages
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let mut last_seen = self.last_seen.lock().await;
        for folder_id in &self.config.folder_ids {
            last_seen.insert(folder_id.clone(), now.clone());
        }

        tracing::info!(
            folders = ?self.config.folder_ids,
            "Email channel connected"
        );
        Ok(())
    }

    async fn poll_messages(&self) -> anyhow::Result<Vec<InboundMessage>> {
        let mut all_messages = Vec::new();
        let mut last_seen = self.last_seen.lock().await;

        for folder_id in &self.config.folder_ids {
            let after = last_seen
                .get(folder_id)
                .cloned()
                .unwrap_or_else(|| Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));

            let messages = self.client.get_messages(folder_id, &after, 50).await?;

            if messages.is_empty() {
                continue;
            }

            // Advance last_seen to the newest message in this folder
            if let Some(newest) = messages
                .iter()
                .filter_map(|m| m.received_date_time.as_deref())
                .max()
            {
                last_seen.insert(folder_id.clone(), newest.to_string());
            }

            for msg in messages {
                // Apply sender filter
                let sender_address = msg
                    .from
                    .as_ref()
                    .map(|r| r.email_address.address.as_str())
                    .unwrap_or("");

                if !self.matches_sender_filter(sender_address) {
                    tracing::debug!(
                        sender = sender_address,
                        message_id = %msg.id,
                        "Email filtered out by sender filter"
                    );
                    continue;
                }

                // Apply subject filter
                let subject = msg.subject.as_deref().unwrap_or("");
                if !self.matches_subject_filter(subject) {
                    tracing::debug!(
                        subject,
                        message_id = %msg.id,
                        "Email filtered out by subject filter"
                    );
                    continue;
                }

                // Extract body text
                let body_text = Self::extract_body_text(&msg);

                // Mark as read to avoid re-processing
                if let Err(e) = self.client.mark_as_read(&msg.id).await {
                    tracing::warn!(
                        message_id = %msg.id,
                        error = %e,
                        "Failed to mark email as read"
                    );
                }

                all_messages.push(msg.to_inbound_message(&body_text));
            }
        }

        Ok(all_messages)
    }

    async fn send_message(&self, msg: OutboundMessage) -> anyhow::Result<()> {
        // reply_to contains "message_id:sender_address" for threading
        let reply_to = msg
            .reply_to
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Email send_message requires reply_to"))?;

        if let Some((message_id, _address)) = reply_to.split_once(':') {
            // Reply to the original message for proper threading
            self.client
                .reply_to_message(message_id, &msg.content)
                .await
        } else {
            // Treat as a new message to the address
            self.client
                .send_mail(reply_to, "Re:", &msg.content)
                .await
        }
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        tracing::info!("Email channel disconnecting");
        // Clear last-seen state
        self.last_seen.lock().await.clear();
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Poll Loop ───────────────────────────────────────────────────

/// Run the continuous poll loop as a tokio task.
///
/// Periodically polls configured mail folders and pushes
/// `Trigger::Message` into the mpsc channel. Uses exponential backoff
/// on consecutive errors (doubles interval, caps at 5 minutes).
/// Exits when the trigger receiver is dropped (daemon shutting down).
pub async fn run_poll_loop(channel: EmailChannel) {
    let poll_interval = Duration::from_secs(channel.config.poll_interval_secs);
    let mut current_interval = poll_interval;
    let max_backoff = Duration::from_secs(300); // 5 minutes

    loop {
        tokio::time::sleep(current_interval).await;

        match channel.poll_messages().await {
            Ok(messages) => {
                // Reset to base interval on success
                current_interval = poll_interval;
                for msg in messages {
                    if let Err(e) = channel.trigger_tx.send(Trigger::Message(msg)).await {
                        tracing::error!("Failed to send email trigger: {e}");
                        return; // Receiver dropped, daemon shutting down
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Email poll error: {e}, retrying in {current_interval:?}"
                );
                // Exponential backoff: double interval, cap at 5 minutes
                current_interval = (current_interval * 2).min(max_backoff);
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::email::types::{GraphMailMessage, MailBody};

    /// Helper to create an EmailChannel with dummy config for testing filters.
    fn make_channel(sender_filter: Vec<String>, subject_filter: Vec<String>) -> EmailChannel {
        let auth = Arc::new(crate::channels::teams::oauth::MsGraphAuth::new("t", "c", "s"));
        let client = EmailClient::new(auth);
        let config = EmailConfig {
            enabled: true,
            poll_interval_secs: 60,
            folder_ids: vec!["Inbox".to_string()],
            sender_filter,
            subject_filter,
        };
        let (tx, _rx) = mpsc::channel(1);
        EmailChannel::new(client, config, tx)
    }

    // ── Sender Filter Tests ──────────────────────────────

    #[test]
    fn sender_filter_empty_allows_all() {
        let ch = make_channel(vec![], vec![]);
        assert!(ch.matches_sender_filter("anyone@anywhere.com"));
    }

    #[test]
    fn sender_filter_exact_match() {
        let ch = make_channel(vec!["john@example.com".to_string()], vec![]);
        assert!(ch.matches_sender_filter("john@example.com"));
        assert!(ch.matches_sender_filter("John@Example.com")); // case insensitive
        assert!(!ch.matches_sender_filter("jane@example.com"));
    }

    #[test]
    fn sender_filter_domain_with_at() {
        let ch = make_channel(vec!["@example.com".to_string()], vec![]);
        assert!(ch.matches_sender_filter("anyone@example.com"));
        assert!(ch.matches_sender_filter("boss@example.com"));
        assert!(!ch.matches_sender_filter("someone@other.com"));
    }

    #[test]
    fn sender_filter_domain_without_at() {
        let ch = make_channel(vec!["example.com".to_string()], vec![]);
        assert!(ch.matches_sender_filter("anyone@example.com"));
        assert!(!ch.matches_sender_filter("someone@other.com"));
    }

    #[test]
    fn sender_filter_multiple_entries() {
        let ch = make_channel(
            vec!["@company.com".to_string(), "boss@external.com".to_string()],
            vec![],
        );
        assert!(ch.matches_sender_filter("alice@company.com"));
        assert!(ch.matches_sender_filter("boss@external.com"));
        assert!(!ch.matches_sender_filter("random@other.com"));
    }

    // ── Subject Filter Tests ─────────────────────────────

    #[test]
    fn subject_filter_empty_allows_all() {
        let ch = make_channel(vec![], vec![]);
        assert!(ch.matches_subject_filter("Any subject"));
    }

    #[test]
    fn subject_filter_substring_match() {
        let ch = make_channel(vec![], vec!["urgent".to_string()]);
        assert!(ch.matches_subject_filter("URGENT: Please review"));
        assert!(ch.matches_subject_filter("This is urgent"));
        assert!(!ch.matches_subject_filter("Normal email"));
    }

    #[test]
    fn subject_filter_multiple_entries() {
        let ch = make_channel(
            vec![],
            vec!["urgent".to_string(), "action required".to_string()],
        );
        assert!(ch.matches_subject_filter("URGENT: Something"));
        assert!(ch.matches_subject_filter("Action Required: Review"));
        assert!(!ch.matches_subject_filter("FYI: Newsletter"));
    }

    // ── Body Extraction Tests ────────────────────────────

    #[test]
    fn extract_body_html() {
        let msg = GraphMailMessage {
            id: "msg-1".to_string(),
            subject: None,
            body_preview: Some("preview".to_string()),
            body: Some(MailBody {
                content: "<p>Hello <b>world</b></p>".to_string(),
                content_type: Some("html".to_string()),
            }),
            from: None,
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };
        let text = EmailChannel::extract_body_text(&msg);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn extract_body_plain_text() {
        let msg = GraphMailMessage {
            id: "msg-2".to_string(),
            subject: None,
            body_preview: None,
            body: Some(MailBody {
                content: "Plain text body".to_string(),
                content_type: Some("text".to_string()),
            }),
            from: None,
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };
        let text = EmailChannel::extract_body_text(&msg);
        assert_eq!(text, "Plain text body");
    }

    #[test]
    fn extract_body_falls_back_to_preview() {
        let msg = GraphMailMessage {
            id: "msg-3".to_string(),
            subject: None,
            body_preview: Some("Preview text".to_string()),
            body: None,
            from: None,
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };
        let text = EmailChannel::extract_body_text(&msg);
        assert_eq!(text, "Preview text");
    }

    #[test]
    fn extract_body_no_body_or_preview() {
        let msg = GraphMailMessage {
            id: "msg-4".to_string(),
            subject: None,
            body_preview: None,
            body: None,
            from: None,
            received_date_time: None,
            conversation_id: None,
            is_read: None,
        };
        let text = EmailChannel::extract_body_text(&msg);
        assert_eq!(text, "");
    }

    // ── Backoff Tests ────────────────────────────────────

    #[test]
    fn backoff_doubles_and_caps() {
        let base = Duration::from_secs(60);
        let max = Duration::from_secs(300);

        let mut interval = base;

        // First error: 60 -> 120
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(120));

        // Second error: 120 -> 240
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(240));

        // Third error: 240 -> 300 (capped)
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(300));

        // Fourth: stays at 300
        interval = (interval * 2).min(max);
        assert_eq!(interval, Duration::from_secs(300));

        // Reset on success
        interval = base;
        assert_eq!(interval, Duration::from_secs(60));
    }
}
