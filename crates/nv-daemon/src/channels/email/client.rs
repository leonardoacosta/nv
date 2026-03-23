use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use reqwest::Client;

use crate::channels::teams::oauth::MsGraphAuth;

use super::types::{
    GraphMailFolder, GraphMailListResponse, GraphMailMessage, SendMailAddress, SendMailBody,
    SendMailMessage, SendMailRecipient, SendMailRequest,
};

/// MS Graph API base URL (v1.0).
const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";

/// Maximum number of rate-limit retries before giving up.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// REST client for MS Graph Mail API.
///
/// All requests use a Bearer token from the shared `MsGraphAuth` instance.
/// Handles 429 rate limiting with Retry-After header.
pub struct EmailClient {
    http: Client,
    auth: Arc<MsGraphAuth>,
}

impl EmailClient {
    /// Create a new Email REST client with a shared auth instance.
    pub fn new(auth: Arc<MsGraphAuth>) -> Self {
        Self {
            http: Client::new(),
            auth,
        }
    }

    /// Get a reference to the underlying auth instance.
    pub fn auth(&self) -> &Arc<MsGraphAuth> {
        &self.auth
    }

    // ── Mail Folder Operations ──────────────────────────────────

    /// List mail folders for the authenticated user.
    #[allow(dead_code)]
    pub async fn list_folders(&self) -> anyhow::Result<Vec<GraphMailFolder>> {
        let url = format!("{GRAPH_API_BASE}/me/mailFolders");
        let token = self.auth.get_token().await?;

        let resp = self.http.get(&url).bearer_auth(&token).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("List mail folders failed ({}): {}", status, body);
        }

        let list: GraphMailListResponse<GraphMailFolder> = resp.json().await?;
        Ok(list.value)
    }

    // ── Message Operations ──────────────────────────────────────

    /// Fetch messages from a mail folder, optionally filtered by received time.
    ///
    /// - `folder_id`: The folder ID or well-known name (e.g., "Inbox").
    /// - `after`: ISO 8601 datetime string. Only messages received after this time are returned.
    /// - `top`: Maximum number of messages to return.
    pub async fn get_messages(
        &self,
        folder_id: &str,
        after: &str,
        top: u32,
    ) -> anyhow::Result<Vec<GraphMailMessage>> {
        let url = format!(
            "{GRAPH_API_BASE}/me/mailFolders/{folder_id}/messages\
             ?$filter=receivedDateTime gt {after}\
             &$top={top}\
             &$orderby=receivedDateTime asc\
             &$select=id,subject,bodyPreview,body,from,receivedDateTime,conversationId,isRead"
        );

        let token = self.auth.get_token().await?;
        let resp = self.http.get(&url).bearer_auth(&token).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!(
                "Get messages from folder {} failed ({}): {}",
                folder_id,
                status,
                body
            );
        }

        let list: GraphMailListResponse<GraphMailMessage> = resp.json().await?;
        Ok(list.value)
    }

    /// Fetch a single message by ID.
    #[allow(dead_code)]
    pub async fn get_message(&self, message_id: &str) -> anyhow::Result<GraphMailMessage> {
        let url = format!(
            "{GRAPH_API_BASE}/me/messages/{message_id}\
             ?$select=id,subject,bodyPreview,body,from,receivedDateTime,conversationId,isRead"
        );

        let token = self.auth.get_token().await?;
        let resp = self.http.get(&url).bearer_auth(&token).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("Get message {} failed ({}): {}", message_id, status, body);
        }

        let msg: GraphMailMessage = resp.json().await?;
        Ok(msg)
    }

    /// Mark a message as read.
    pub async fn mark_as_read(&self, message_id: &str) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/me/messages/{message_id}");
        let body = serde_json::json!({ "isRead": true });

        let token = self.auth.get_token().await?;
        let resp = self
            .http
            .patch(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let error = resp.text().await.unwrap_or_default();
            bail!("Mark message read failed ({}): {}", status, error);
        }

        Ok(())
    }

    /// Send an email via POST /me/sendMail.
    pub async fn send_mail(
        &self,
        to: &str,
        subject: &str,
        body: &str,
    ) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/me/sendMail");

        let request = SendMailRequest {
            message: SendMailMessage {
                subject: subject.to_string(),
                body: SendMailBody {
                    content_type: "Text".to_string(),
                    content: body.to_string(),
                },
                to_recipients: vec![SendMailRecipient {
                    email_address: SendMailAddress {
                        address: to.to_string(),
                    },
                }],
            },
            save_to_sent_items: true,
        };

        self.post_with_retry(&url, &serde_json::to_value(&request)?).await
    }

    /// Reply to an existing message for proper threading.
    pub async fn reply_to_message(
        &self,
        message_id: &str,
        comment: &str,
    ) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/me/messages/{message_id}/reply");

        let body = serde_json::json!({
            "comment": comment,
        });

        self.post_with_retry(&url, &body).await
    }

    // ── Internal Helpers ────────────────────────────────────────

    /// POST with rate-limit retry (429 + Retry-After handling).
    async fn post_with_retry(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> anyhow::Result<()> {
        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let token = self.auth.get_token().await?;
            let resp = self
                .http
                .post(url)
                .bearer_auth(&token)
                .json(body)
                .send()
                .await?;

            let status = resp.status();

            // sendMail returns 202 Accepted (not 200)
            if status.is_success() || status.as_u16() == 202 {
                return Ok(());
            }

            if status.as_u16() == 429 {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);

                if attempt < MAX_RATE_LIMIT_RETRIES {
                    tracing::warn!(
                        retry_after_secs = retry_after,
                        attempt = attempt + 1,
                        "MS Graph Mail rate limited, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                } else {
                    bail!("MS Graph Mail rate limited after {MAX_RATE_LIMIT_RETRIES} retries");
                }
            }

            let error_text = resp.text().await.unwrap_or_default();
            bail!("MS Graph Mail API error ({}): {}", status, error_text);
        }

        bail!("MS Graph Mail post_with_retry exhausted retries")
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_api_base_is_v1() {
        assert!(GRAPH_API_BASE.contains("v1.0"));
        assert!(GRAPH_API_BASE.starts_with("https://"));
    }

    #[test]
    fn send_mail_request_body_format() {
        let req = SendMailRequest {
            message: SendMailMessage {
                subject: "Test Subject".to_string(),
                body: SendMailBody {
                    content_type: "Text".to_string(),
                    content: "Test body".to_string(),
                },
                to_recipients: vec![SendMailRecipient {
                    email_address: SendMailAddress {
                        address: "recipient@example.com".to_string(),
                    },
                }],
            },
            save_to_sent_items: true,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["message"]["subject"], "Test Subject");
        assert_eq!(json["message"]["body"]["contentType"], "Text");
        assert_eq!(json["message"]["body"]["content"], "Test body");
        assert_eq!(json["saveToSentItems"], true);
        assert_eq!(
            json["message"]["toRecipients"][0]["emailAddress"]["address"],
            "recipient@example.com"
        );
    }

    #[test]
    fn reply_body_format() {
        let body = serde_json::json!({
            "comment": "Thank you for your email.",
        });
        assert_eq!(body["comment"], "Thank you for your email.");
    }
}
