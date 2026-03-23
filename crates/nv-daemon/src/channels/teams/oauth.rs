use std::time::{Duration, Instant};

use reqwest::Client;
use tokio::sync::Mutex;

use super::types::OAuthTokenResponse;

/// Buffer before token expiry to trigger refresh (5 minutes).
const REFRESH_BUFFER: Duration = Duration::from_secs(300);

/// Cached token state.
#[derive(Debug)]
struct TokenState {
    access_token: String,
    expires_at: Instant,
}

/// MS Graph OAuth2 client credentials authenticator.
///
/// Handles token acquisition and automatic refresh. The token is cached
/// in memory and refreshed 5 minutes before expiry. Thread-safe via Mutex.
///
/// Designed as a reusable struct so future MS Graph integrations (e.g.,
/// email channel) can share the same auth client.
#[derive(Debug)]
pub struct MsGraphAuth {
    http: Client,
    tenant_id: String,
    client_id: String,
    client_secret: String,
    token: Mutex<Option<TokenState>>,
}

impl MsGraphAuth {
    /// Create a new authenticator.
    ///
    /// Does NOT acquire a token immediately -- call `get_token()` or
    /// `authenticate()` to trigger the first token request.
    pub fn new(tenant_id: &str, client_id: &str, client_secret: &str) -> Self {
        Self {
            http: Client::new(),
            tenant_id: tenant_id.to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            token: Mutex::new(None),
        }
    }

    /// Get a valid access token, refreshing if necessary.
    ///
    /// This is the primary method callers should use. It checks the cached
    /// token's expiry and only requests a new one when needed.
    pub async fn get_token(&self) -> anyhow::Result<String> {
        let mut guard = self.token.lock().await;

        if let Some(ref state) = *guard {
            if Instant::now() + REFRESH_BUFFER < state.expires_at {
                return Ok(state.access_token.clone());
            }
            tracing::debug!("MS Graph token near expiry, refreshing");
        }

        let resp = self.request_token().await?;
        let expires_at = Instant::now() + Duration::from_secs(resp.expires_in);

        let token = resp.access_token.clone();
        *guard = Some(TokenState {
            access_token: resp.access_token,
            expires_at,
        });

        tracing::info!(
            expires_in_secs = resp.expires_in,
            "MS Graph OAuth token acquired"
        );

        Ok(token)
    }

    /// Acquire the initial token. Convenience wrapper around `get_token()`.
    pub async fn authenticate(&self) -> anyhow::Result<()> {
        self.get_token().await?;
        Ok(())
    }

    /// Drop the cached token (used on disconnect).
    pub async fn clear_token(&self) {
        *self.token.lock().await = None;
    }

    /// Request a new token from the Microsoft Identity Platform.
    async fn request_token(&self) -> anyhow::Result<OAuthTokenResponse> {
        let url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("scope", "https://graph.microsoft.com/.default"),
        ];

        let resp = self
            .http
            .post(&url)
            .form(&params)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("OAuth token request failed ({}): {}", status, body);
        }

        let token_resp: OAuthTokenResponse = resp.json().await?;
        Ok(token_resp)
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_buffer_is_5_minutes() {
        assert_eq!(REFRESH_BUFFER, Duration::from_secs(300));
    }

    #[test]
    fn auth_creates_without_token() {
        let auth = MsGraphAuth::new("tenant-1", "client-1", "secret-1");
        assert_eq!(auth.tenant_id, "tenant-1");
        assert_eq!(auth.client_id, "client-1");
        assert_eq!(auth.client_secret, "secret-1");
    }

    #[tokio::test]
    async fn clear_token_removes_cached() {
        let auth = MsGraphAuth::new("t", "c", "s");

        // Manually inject a token
        {
            let mut guard = auth.token.lock().await;
            *guard = Some(TokenState {
                access_token: "test-token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            });
        }

        // Verify it's cached
        {
            let guard = auth.token.lock().await;
            assert!(guard.is_some());
        }

        auth.clear_token().await;

        // Verify it's cleared
        {
            let guard = auth.token.lock().await;
            assert!(guard.is_none());
        }
    }

    #[tokio::test]
    async fn get_token_returns_cached_when_valid() {
        let auth = MsGraphAuth::new("t", "c", "s");

        // Inject a token valid for 1 hour
        {
            let mut guard = auth.token.lock().await;
            *guard = Some(TokenState {
                access_token: "cached-token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            });
        }

        // Should return cached token without network call
        let token = auth.get_token().await.unwrap();
        assert_eq!(token, "cached-token");
    }

    #[tokio::test]
    async fn get_token_refreshes_when_near_expiry() {
        let auth = MsGraphAuth::new("t", "c", "s");

        // Inject a token that expires in 2 minutes (within 5-minute buffer)
        {
            let mut guard = auth.token.lock().await;
            *guard = Some(TokenState {
                access_token: "expiring-token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(120),
            });
        }

        // Should try to refresh (will fail since no real endpoint)
        let result = auth.get_token().await;
        assert!(result.is_err()); // Expected: no real OAuth server
    }
}
