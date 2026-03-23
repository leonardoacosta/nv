//! HTTP client wrapping the BlueBubbles REST API (v1).
//!
//! All requests include the `password` query parameter for authentication.
//! The client is `Clone`-able since `reqwest::Client` uses an inner `Arc`.

use anyhow::Context;
use reqwest::Client;

use super::types::{BbMessageResponse, BbSendResponse};

/// HTTP client for the BlueBubbles server REST API.
#[derive(Debug, Clone)]
pub struct BlueBubblesClient {
    http: Client,
    base_url: String,
    password: String,
}

impl BlueBubblesClient {
    /// Create a new BlueBubbles API client.
    ///
    /// - `base_url`: BlueBubbles server URL (e.g. `http://mac.tailnet:1234`).
    /// - `password`: The server password configured in BlueBubbles.
    pub fn new(base_url: &str, password: &str) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            password: password.to_string(),
        }
    }

    /// Fetch messages created after the given timestamp.
    ///
    /// - `after`: Unix timestamp in milliseconds. Only messages with
    ///   `dateCreated > after` are returned.
    /// - `limit`: Maximum number of messages to return.
    ///
    /// Calls `GET /api/v1/message?after=<ts>&limit=<n>&sort=ASC&password=<pw>`.
    pub async fn get_messages(
        &self,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<super::types::BbMessage>> {
        let url = format!("{}/api/v1/message", self.base_url);

        let resp = self
            .http
            .get(&url)
            .query(&[
                ("after", after.to_string()),
                ("limit", limit.to_string()),
                ("sort", "ASC".to_string()),
                ("password", self.password.clone()),
            ])
            .send()
            .await
            .context("BlueBubbles GET /api/v1/message request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "BlueBubbles GET /api/v1/message returned {status}: {body}"
            );
        }

        let body: BbMessageResponse = resp
            .json()
            .await
            .context("Failed to parse BlueBubbles message response")?;

        Ok(body.data)
    }

    /// Send a text message to a chat.
    ///
    /// - `chat_guid`: The chat GUID (e.g. `iMessage;-;+15551234567`).
    /// - `text`: The message text to send.
    ///
    /// Calls `POST /api/v1/message/text?password=<pw>`.
    pub async fn send_message(&self, chat_guid: &str, text: &str) -> anyhow::Result<()> {
        let url = format!("{}/api/v1/message/text", self.base_url);

        let temp_guid = uuid::Uuid::new_v4().to_string();

        let resp = self
            .http
            .post(&url)
            .query(&[("password", &self.password)])
            .json(&serde_json::json!({
                "chatGuid": chat_guid,
                "message": text,
                "tempGuid": temp_guid,
            }))
            .send()
            .await
            .context("BlueBubbles POST /api/v1/message/text request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "BlueBubbles POST /api/v1/message/text returned {status}: {body}"
            );
        }

        let send_resp: BbSendResponse = resp
            .json()
            .await
            .context("Failed to parse BlueBubbles send response")?;

        if send_resp.status != 200 {
            anyhow::bail!(
                "BlueBubbles send failed with status {}: {}",
                send_resp.status,
                send_resp.message.unwrap_or_default()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_trims_trailing_slash() {
        let client = BlueBubblesClient::new("http://mac.local:1234/", "secret");
        assert_eq!(client.base_url, "http://mac.local:1234");
    }

    #[test]
    fn client_preserves_clean_url() {
        let client = BlueBubblesClient::new("http://mac.local:1234", "pw123");
        assert_eq!(client.base_url, "http://mac.local:1234");
        assert_eq!(client.password, "pw123");
    }
}
