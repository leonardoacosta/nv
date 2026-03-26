use std::path::Path;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::types::OAuthTokenResponse;

/// Buffer before token expiry to trigger refresh (5 minutes).
const REFRESH_BUFFER: Duration = Duration::from_secs(300);

/// Delegated (user) permission scopes for all Graph features requiring user context.
/// Includes Outlook (Mail, Calendar) and Teams Chat access.
const DELEGATED_SCOPES: &str =
    "https://graph.microsoft.com/Mail.Read https://graph.microsoft.com/Calendars.Read \
     https://graph.microsoft.com/Chat.Read https://graph.microsoft.com/User.Read offline_access";

/// Device-code poll timeout (5 minutes).
const DEVICE_CODE_TIMEOUT: Duration = Duration::from_secs(300);

// ── Token Cache Serialization ─────────────────────────────────────────

/// On-disk representation of a cached user token.
#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    /// Unix timestamp (seconds) when the access token expires.
    expires_at_unix: u64,
    client_id: String,
    tenant_id: String,
}

// ── Device-Code Response Types ────────────────────────────────────────

/// Response from the /devicecode endpoint.
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    /// Polling interval in seconds.
    interval: u64,
    #[allow(dead_code)]
    expires_in: u64,
    #[allow(dead_code)]
    message: Option<String>,
}

/// Response from token endpoint during device-code polling.
#[derive(Debug, Deserialize)]
struct DeviceCodeTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}

// ── MsGraphUserAuth ───────────────────────────────────────────────────

/// Device-code / refresh-token authenticator for delegated (user) MS Graph permissions.
///
/// Used by Outlook tools (`read_outlook_inbox`, `read_outlook_calendar`) which require
/// `Mail.Read` and `Calendars.Read` delegated permissions. These cannot be acquired by
/// client-credentials; the user must authenticate interactively on first use.
///
/// Token cache: `~/.config/nv/graph-token.json` (or `NV_GRAPH_TOKEN_PATH` env var).
#[derive(Debug)]
pub struct MsGraphUserAuth {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the access token expires.
    expires_at: Instant,
    pub client_id: String,
    pub tenant_id: String,
    http: Client,
}

impl MsGraphUserAuth {
    /// Load a cached token from disk.
    ///
    /// Returns `None` if the file is missing, cannot be parsed, or the token is
    /// expired with no refresh token available.
    pub fn from_cache(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let cache: TokenCache = serde_json::from_str(&content).ok()?;

        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Convert unix expiry to an Instant
        let secs_remaining = cache.expires_at_unix.saturating_sub(now_unix);
        let expires_at = Instant::now() + Duration::from_secs(secs_remaining);

        // If expired and no refresh token, caller needs to re-authenticate
        if secs_remaining == 0 && cache.refresh_token.is_none() {
            return None;
        }

        Some(Self {
            access_token: cache.access_token,
            refresh_token: cache.refresh_token,
            expires_at,
            client_id: cache.client_id,
            tenant_id: cache.tenant_id,
            http: Client::new(),
        })
    }

    /// Run interactive device-code flow to acquire delegated tokens.
    ///
    /// Prints the user code and verification URL to stderr so the daemon can relay
    /// the instructions to the user. Polls the token endpoint until the user
    /// completes the flow or the 5-minute timeout elapses.
    pub async fn device_code_flow(
        client_id: &str,
        tenant_id: &str,
        scopes: &str,
    ) -> anyhow::Result<Self> {
        let http = Client::new();

        // Step 1: request device code
        let dc_url = format!(
            "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/devicecode"
        );
        let dc_resp = http
            .post(&dc_url)
            .form(&[("client_id", client_id), ("scope", scopes)])
            .send()
            .await?;

        let dc_status = dc_resp.status();
        if !dc_status.is_success() {
            let body = dc_resp.text().await.unwrap_or_default();
            anyhow::bail!("Device code request failed ({dc_status}): {body}");
        }

        let dc: DeviceCodeResponse = dc_resp.json().await?;

        // Print instructions to stderr
        eprintln!(
            "\n[Nova] Graph API authentication required.\n\
             Visit: {}\n\
             Enter code: {}\n\
             Waiting for authentication...",
            dc.verification_uri, dc.user_code
        );

        // Step 2: poll token endpoint
        let token_url = format!(
            "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"
        );
        let poll_interval = Duration::from_secs(dc.interval.max(5));
        let deadline = Instant::now() + DEVICE_CODE_TIMEOUT;

        loop {
            tokio::time::sleep(poll_interval).await;

            if Instant::now() > deadline {
                anyhow::bail!("Device code authentication timed out. Run `nv auth graph` to try again.");
            }

            let poll_resp = http
                .post(&token_url)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", client_id),
                    ("device_code", &dc.device_code),
                ])
                .send()
                .await?;

            let token_data: DeviceCodeTokenResponse = poll_resp.json().await?;

            match token_data.error.as_deref() {
                None => {
                    // Success
                    let access_token = token_data
                        .access_token
                        .ok_or_else(|| anyhow::anyhow!("Token response missing access_token"))?;
                    let expires_in = token_data.expires_in.unwrap_or(3600);
                    let expires_at = Instant::now() + Duration::from_secs(expires_in);

                    eprintln!("[Nova] Graph API authenticated successfully.");

                    return Ok(Self {
                        access_token,
                        refresh_token: token_data.refresh_token,
                        expires_at,
                        client_id: client_id.to_string(),
                        tenant_id: tenant_id.to_string(),
                        http,
                    });
                }
                Some("authorization_pending") | Some("slow_down") => {
                    // Keep polling
                    continue;
                }
                Some("authorization_declined") => {
                    anyhow::bail!("Authentication declined by user.");
                }
                Some("expired_token") => {
                    anyhow::bail!("Device code expired. Run `nv auth graph` to try again.");
                }
                Some(err) => {
                    anyhow::bail!("Authentication error: {err}");
                }
            }
        }
    }

    /// Get a valid access token, silently refreshing if near expiry.
    ///
    /// Returns an error if the token is expired and refresh fails, directing
    /// the user to run `nv auth graph`.
    pub async fn get_token(&self) -> anyhow::Result<String> {
        // Token still valid (with 5-minute buffer)
        if Instant::now() + REFRESH_BUFFER < self.expires_at {
            return Ok(self.access_token.clone());
        }

        // Attempt silent refresh
        if let Some(ref refresh_token) = self.refresh_token {
            let token_url = format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                self.tenant_id
            );
            let resp = self
                .http
                .post(&token_url)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("client_id", &self.client_id),
                    ("refresh_token", refresh_token),
                    ("scope", DELEGATED_SCOPES),
                ])
                .send()
                .await?;

            if resp.status().is_success() {
                let token_data: OAuthTokenResponse = resp.json().await?;
                return Ok(token_data.access_token);
            }
        }

        anyhow::bail!(
            "Graph token expired — run `nv auth graph` to re-authenticate"
        )
    }

    /// Persist the token to disk with mode 0o600.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Approximate expires_at_unix from remaining Instant duration
        let remaining = self.expires_at.saturating_duration_since(Instant::now());
        let expires_at_unix = now_unix + remaining.as_secs();

        let cache = TokenCache {
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            expires_at_unix,
            client_id: self.client_id.clone(),
            tenant_id: self.tenant_id.clone(),
        };

        let json = serde_json::to_string_pretty(&cache)?;
        std::fs::write(path, &json)?;

        // Restrict permissions to owner-only (0o600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Convenience: load from cache or run device-code flow.
    ///
    /// Reads credentials from env vars `MS_GRAPH_CLIENT_ID` and
    /// `MS_GRAPH_TENANT_ID`. Cache path: `~/.config/nv/graph-token.json`
    /// (or `NV_GRAPH_TOKEN_PATH` env var).
    pub async fn try_load_or_prompt() -> anyhow::Result<Self> {
        let cache_path = graph_token_path();

        if let Some(auth) = Self::from_cache(&cache_path) {
            // Validate token is usable (not expired with no refresh)
            match auth.get_token().await {
                Ok(_) => return Ok(auth),
                Err(_) => {
                    tracing::debug!("Cached graph token invalid, re-authenticating via device code");
                }
            }
        }

        let client_id = std::env::var("MS_GRAPH_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("MS Graph not configured — MS_GRAPH_CLIENT_ID env var not set"))?;
        let tenant_id = std::env::var("MS_GRAPH_TENANT_ID")
            .map_err(|_| anyhow::anyhow!("MS Graph not configured — MS_GRAPH_TENANT_ID env var not set"))?;

        let auth = Self::device_code_flow(&client_id, &tenant_id, DELEGATED_SCOPES).await?;
        auth.save(&cache_path)?;
        Ok(auth)
    }
}

/// Resolve the graph token cache path from env or default.
pub fn graph_token_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("NV_GRAPH_TOKEN_PATH") {
        return std::path::PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    std::path::PathBuf::from(home)
        .join(".config")
        .join("nv")
        .join("graph-token.json")
}

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

    // ── MsGraphUserAuth Tests ─────────────────────────────────────────

    #[test]
    fn user_auth_from_cache_missing_file() {
        let result = MsGraphUserAuth::from_cache(std::path::Path::new("/nonexistent/path/token.json"));
        assert!(result.is_none());
    }

    #[test]
    fn user_auth_save_and_load_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("graph-token.json");

        let auth = MsGraphUserAuth {
            access_token: "test-access-token".to_string(),
            refresh_token: Some("test-refresh-token".to_string()),
            expires_at: Instant::now() + Duration::from_secs(3600),
            client_id: "client-id-123".to_string(),
            tenant_id: "tenant-id-456".to_string(),
            http: Client::new(),
        };

        auth.save(&path).unwrap();

        // Verify file was created
        assert!(path.exists());

        // Check permissions (unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        // Load from cache
        let loaded = MsGraphUserAuth::from_cache(&path).unwrap();
        assert_eq!(loaded.access_token, "test-access-token");
        assert_eq!(loaded.refresh_token.as_deref(), Some("test-refresh-token"));
        assert_eq!(loaded.client_id, "client-id-123");
        assert_eq!(loaded.tenant_id, "tenant-id-456");
    }

    #[tokio::test]
    async fn user_auth_get_token_returns_cached_when_valid() {
        let auth = MsGraphUserAuth {
            access_token: "valid-token".to_string(),
            refresh_token: None,
            expires_at: Instant::now() + Duration::from_secs(3600),
            client_id: "c".to_string(),
            tenant_id: "t".to_string(),
            http: Client::new(),
        };

        let token = auth.get_token().await.unwrap();
        assert_eq!(token, "valid-token");
    }

    #[tokio::test]
    async fn user_auth_get_token_errors_when_expired_no_refresh() {
        let auth = MsGraphUserAuth {
            access_token: "expired-token".to_string(),
            refresh_token: None,
            // Token expired 1 hour ago
            expires_at: Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or(Instant::now()),
            client_id: "c".to_string(),
            tenant_id: "t".to_string(),
            http: Client::new(),
        };

        let result = auth.get_token().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nv auth graph"));
    }

    #[test]
    fn graph_token_path_default() {
        let path = graph_token_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".config/nv/graph-token.json") || path_str.contains("NV_GRAPH_TOKEN_PATH"));
    }

    #[test]
    fn user_auth_from_cache_expired_no_refresh_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("graph-token.json");

        // Write a token that has already expired with no refresh token
        let cache = serde_json::json!({
            "access_token": "expired-token",
            "refresh_token": null,
            "expires_at_unix": 1000u64,  // very old Unix timestamp
            "client_id": "c",
            "tenant_id": "t"
        });
        std::fs::write(&path, serde_json::to_string(&cache).unwrap()).unwrap();

        let result = MsGraphUserAuth::from_cache(&path);
        assert!(result.is_none());
    }
}
