use std::sync::Arc;
use std::time::Duration;

use anyhow::bail;
use reqwest::Client;

use super::oauth::MsGraphAuth;
use super::types::{
    ChatMessage, GraphListResponse, SubscriptionRequest, SubscriptionResponse, TeamChannel,
};

/// MS Graph API base URL (v1.0).
const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";

/// Maximum number of rate-limit retries before giving up.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// REST client for MS Graph Teams API.
///
/// All requests use a Bearer token from the shared `MsGraphAuth` instance.
/// Handles 429 rate limiting with Retry-After header.
pub struct TeamsClient {
    http: Client,
    auth: Arc<MsGraphAuth>,
}

impl TeamsClient {
    /// Create a new Teams REST client.
    pub fn new(auth: Arc<MsGraphAuth>) -> Self {
        Self {
            http: Client::new(),
            auth,
        }
    }

    // ── Subscription Management ──────────────────────────────────

    /// Create a subscription for channel message notifications.
    ///
    /// MS Graph subscriptions have a max lifetime of 60 minutes for
    /// channel messages. The caller is responsible for renewing before expiry.
    pub async fn create_subscription(
        &self,
        notification_url: &str,
        resource: &str,
        expiration: &str,
        client_state: Option<&str>,
    ) -> anyhow::Result<SubscriptionResponse> {
        let url = format!("{GRAPH_API_BASE}/subscriptions");

        let body = SubscriptionRequest {
            change_type: "created".to_string(),
            notification_url: notification_url.to_string(),
            resource: resource.to_string(),
            expiration_date_time: expiration.to_string(),
            client_state: client_state.map(|s| s.to_string()),
        };

        let token = self.auth.get_token().await?;
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("Create subscription failed ({}): {}", status, body);
        }

        let sub: SubscriptionResponse = resp.json().await?;
        tracing::info!(
            subscription_id = %sub.id,
            resource = %sub.resource,
            expires = %sub.expiration_date_time,
            "MS Graph subscription created"
        );
        Ok(sub)
    }

    /// Renew an existing subscription by updating its expiration time.
    #[allow(dead_code)]
    pub async fn renew_subscription(
        &self,
        subscription_id: &str,
        new_expiration: &str,
    ) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/subscriptions/{subscription_id}");

        let body = serde_json::json!({
            "expirationDateTime": new_expiration,
        });

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
            let body = resp.text().await.unwrap_or_default();
            bail!("Renew subscription failed ({}): {}", status, body);
        }

        tracing::info!(
            subscription_id,
            new_expiration,
            "MS Graph subscription renewed"
        );
        Ok(())
    }

    /// Delete a subscription.
    pub async fn delete_subscription(&self, subscription_id: &str) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/subscriptions/{subscription_id}");

        let token = self.auth.get_token().await?;
        let resp = self
            .http
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 404 {
            let body = resp.text().await.unwrap_or_default();
            bail!("Delete subscription failed ({}): {}", status, body);
        }

        tracing::info!(subscription_id, "MS Graph subscription deleted");
        Ok(())
    }

    // ── Message Operations ───────────────────────────────────────

    /// Send a message to a Teams channel.
    pub async fn send_channel_message(
        &self,
        team_id: &str,
        channel_id: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        let url = format!(
            "{GRAPH_API_BASE}/teams/{team_id}/channels/{channel_id}/messages"
        );

        let body = serde_json::json!({
            "body": {
                "content": content,
                "contentType": "text",
            }
        });

        self.post_with_retry(&url, &body).await
    }

    /// Send a message to a Teams chat (direct message / group chat).
    pub async fn send_chat_message(
        &self,
        chat_id: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        let url = format!("{GRAPH_API_BASE}/chats/{chat_id}/messages");

        let body = serde_json::json!({
            "body": {
                "content": content,
                "contentType": "text",
            }
        });

        self.post_with_retry(&url, &body).await
    }

    /// Fetch a specific message by its resource path.
    ///
    /// Used to retrieve full message content after receiving a change notification.
    pub async fn get_message(&self, resource_path: &str) -> anyhow::Result<ChatMessage> {
        let url = format!("{GRAPH_API_BASE}/{resource_path}");

        let token = self.auth.get_token().await?;
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("Get message failed ({}): {}", status, body);
        }

        let msg: ChatMessage = resp.json().await?;
        Ok(msg)
    }

    // ── Channel Operations ───────────────────────────────────────

    /// List channels in a team.
    #[allow(dead_code)]
    pub async fn list_channels(&self, team_id: &str) -> anyhow::Result<Vec<TeamChannel>> {
        let url = format!("{GRAPH_API_BASE}/teams/{team_id}/channels");

        let token = self.auth.get_token().await?;
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("List channels failed ({}): {}", status, body);
        }

        let list: GraphListResponse<TeamChannel> = resp.json().await?;
        Ok(list.value)
    }

    // ── Internal Helpers ─────────────────────────────────────────

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

            if status.is_success() {
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
                        "MS Graph rate limited, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                } else {
                    bail!("MS Graph rate limited after {MAX_RATE_LIMIT_RETRIES} retries");
                }
            }

            let error_text = resp.text().await.unwrap_or_default();
            bail!("MS Graph API error ({}): {}", status, error_text);
        }

        bail!("MS Graph post_with_retry exhausted retries")
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_api_base_is_v1() {
        assert!(GRAPH_API_BASE.contains("v1.0"));
        assert!(GRAPH_API_BASE.starts_with("https://"));
    }

    #[test]
    fn subscription_request_format() {
        let req = SubscriptionRequest {
            change_type: "created".to_string(),
            notification_url: "https://example.com/webhooks/teams".to_string(),
            resource: "/teams/getAllMessages".to_string(),
            expiration_date_time: "2024-01-01T01:00:00Z".to_string(),
            client_state: Some("nv-secret".to_string()),
        };
        let json = serde_json::to_value(&req).unwrap();
        // Verify camelCase serialization
        assert!(json.get("changeType").is_some());
        assert!(json.get("notificationUrl").is_some());
        assert!(json.get("expirationDateTime").is_some());
        assert!(json.get("clientState").is_some());
    }
}
