//! Microsoft Graph Outlook tools — inbox, calendar, and read email.
//!
//! Three read-only tools using delegated (user) auth via device-code flow:
//! * `outlook_inbox` — list recent messages from a mail folder.
//! * `outlook_calendar` — list upcoming calendar events.
//! * `outlook_read_email` — read the full body of a single message.
//!
//! Auth: `GraphUserAuth` runs device-code flow on first use and caches the
//! token at `~/.config/nv/graph-user-token.json` (or `NV_GRAPH_USER_TOKEN_PATH`).
//! Required env vars: `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_TENANT_ID`.
//! Required API permissions (delegated): `Mail.Read Calendars.Read offline_access`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use nv_core::ToolDefinition;

// ── Constants ─────────────────────────────────────────────────────────

/// MS Graph REST API base URL.
const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";

/// Seconds before expiry to trigger silent refresh.
const REFRESH_BUFFER_SECS: u64 = 60;

/// Device-code flow poll timeout in seconds.
const DEVICE_CODE_TIMEOUT_SECS: u64 = 300;

/// Delegated scopes required for Outlook tools.
const OUTLOOK_SCOPES: &str = "Mail.Read Calendars.Read offline_access";

// ── Token Cache Types ─────────────────────────────────────────────────

/// On-disk token cache for `graph-user-token.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    /// Unix timestamp (seconds) when the access token expires.
    expires_at_unix: u64,
    client_id: String,
    tenant_id: String,
}

// ── Device-Code Flow Types ────────────────────────────────────────────

/// Response from the `/devicecode` endpoint.
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    /// Recommended polling interval in seconds.
    interval: u64,
    #[allow(dead_code)]
    expires_in: u64,
}

/// Response from token endpoint during device-code polling or refresh.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

// ── GraphUserAuth ─────────────────────────────────────────────────────

/// Device-code / refresh-token authenticator for delegated MS Graph permissions.
///
/// Used by Outlook tools which require `Mail.Read` and `Calendars.Read` delegated
/// permissions. These cannot be acquired by client-credentials; the user must
/// authenticate interactively on first use.
///
/// Token cache: `~/.config/nv/graph-user-token.json` (or `NV_GRAPH_USER_TOKEN_PATH`).
/// This path is distinct from `nv-daemon`'s `graph-token.json` to avoid collisions.
#[derive(Debug)]
pub struct GraphUserAuth {
    access_token: String,
    refresh_token: Option<String>,
    /// Unix timestamp (seconds) when the access token expires.
    expires_at_unix: u64,
    client_id: String,
    tenant_id: String,
    http: reqwest::Client,
    /// Path where the token cache is persisted.
    cache_path: PathBuf,
}

impl GraphUserAuth {
    /// Load from cache or run device-code flow.
    ///
    /// Reads `MS_GRAPH_CLIENT_ID` and `MS_GRAPH_TENANT_ID` from env.
    /// Cache path: `NV_GRAPH_USER_TOKEN_PATH` (default `~/.config/nv/graph-user-token.json`).
    ///
    /// On first run, prints device-code instructions to stderr and blocks until
    /// the user authenticates, then saves the acquired token.
    pub async fn from_env_or_cache() -> Result<Self> {
        let cache_path = graph_user_token_path();

        // Try loading from cache first
        if let Some(auth) = Self::from_cache(&cache_path) {
            // If the token is still valid (or has a refresh token), use it
            return Ok(auth);
        }

        // No usable cached token — run device-code flow
        let client_id = std::env::var("MS_GRAPH_CLIENT_ID").map_err(|_| {
            anyhow!("MS Graph not configured — MS_GRAPH_CLIENT_ID env var not set")
        })?;
        let tenant_id = std::env::var("MS_GRAPH_TENANT_ID").map_err(|_| {
            anyhow!("MS Graph not configured — MS_GRAPH_TENANT_ID env var not set")
        })?;

        let auth = Self::device_code_flow(&client_id, &tenant_id, &cache_path).await?;
        Ok(auth)
    }

    /// Attempt to load a token from the cache file.
    ///
    /// Returns `None` if the file is missing, unparseable, or the token is
    /// expired with no refresh token.
    fn from_cache(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let cache: TokenCache = serde_json::from_str(&content).ok()?;

        let now_unix = now_unix_secs();
        // If token is expired and there is no refresh token, the cache is useless
        if cache.expires_at_unix <= now_unix && cache.refresh_token.is_none() {
            return None;
        }

        Some(Self {
            access_token: cache.access_token,
            refresh_token: cache.refresh_token,
            expires_at_unix: cache.expires_at_unix,
            client_id: cache.client_id,
            tenant_id: cache.tenant_id,
            http: reqwest::Client::new(),
            cache_path: path.to_path_buf(),
        })
    }

    /// Return a valid access token, silently refreshing if within the buffer window.
    ///
    /// Updates the on-disk cache when a refresh occurs.
    pub async fn get_token(&mut self) -> Result<String> {
        let now = now_unix_secs();

        // Token still valid with buffer
        if self.expires_at_unix > now + REFRESH_BUFFER_SECS {
            return Ok(self.access_token.clone());
        }

        // Attempt silent refresh
        let refresh_token = self.refresh_token.clone().ok_or_else(|| {
            anyhow!("Graph token expired — run the tool again to re-authenticate via device code")
        })?;

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
                ("refresh_token", &refresh_token),
                ("scope", OUTLOOK_SCOPES),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Graph token refresh failed ({status}): {body}\n\
                 Delete ~/.config/nv/graph-user-token.json and run the tool again to re-authenticate."
            );
        }

        let token_data: TokenResponse = resp.json().await?;
        let new_access = token_data
            .access_token
            .ok_or_else(|| anyhow!("Token refresh response missing access_token"))?;
        let expires_in = token_data.expires_in.unwrap_or(3600);

        // Update in-memory state
        self.access_token = new_access.clone();
        if let Some(new_refresh) = token_data.refresh_token {
            self.refresh_token = Some(new_refresh);
        }
        self.expires_at_unix = now_unix_secs() + expires_in;

        // Persist updated token
        self.save(&self.cache_path.clone())?;

        Ok(new_access)
    }

    /// Persist the token to disk with 0o600 permissions.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let cache = TokenCache {
            access_token: self.access_token.clone(),
            refresh_token: self.refresh_token.clone(),
            expires_at_unix: self.expires_at_unix,
            client_id: self.client_id.clone(),
            tenant_id: self.tenant_id.clone(),
        };

        let json = serde_json::to_string_pretty(&cache)?;
        std::fs::write(path, &json)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Run the OAuth2 device-code flow, save the token, and return `Self`.
    async fn device_code_flow(
        client_id: &str,
        tenant_id: &str,
        cache_path: &Path,
    ) -> Result<Self> {
        let http = reqwest::Client::new();

        // Step 1: Request device code
        let dc_url = format!(
            "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/devicecode"
        );
        let dc_resp = http
            .post(&dc_url)
            .form(&[("client_id", client_id), ("scope", OUTLOOK_SCOPES)])
            .send()
            .await?;

        let dc_status = dc_resp.status();
        if !dc_status.is_success() {
            let body = dc_resp.text().await.unwrap_or_default();
            anyhow::bail!("Device code request failed ({dc_status}): {body}");
        }

        let dc: DeviceCodeResponse = dc_resp.json().await?;

        eprintln!(
            "\n[Nova] Graph API authentication required.\n\
             Visit: {}\n\
             Enter code: {}\n\
             Waiting for authentication...",
            dc.verification_uri, dc.user_code
        );

        // Step 2: Poll token endpoint
        let token_url = format!(
            "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"
        );
        let poll_interval = Duration::from_secs(dc.interval.max(5));
        let deadline = std::time::Instant::now() + Duration::from_secs(DEVICE_CODE_TIMEOUT_SECS);

        loop {
            tokio::time::sleep(poll_interval).await;

            if std::time::Instant::now() > deadline {
                anyhow::bail!(
                    "Device code authentication timed out. Run the tool again to retry."
                );
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

            let token_data: TokenResponse = poll_resp.json().await?;

            match token_data.error.as_deref() {
                None => {
                    let access_token = token_data
                        .access_token
                        .ok_or_else(|| anyhow!("Token response missing access_token"))?;
                    let expires_in = token_data.expires_in.unwrap_or(3600);
                    let expires_at_unix = now_unix_secs() + expires_in;

                    eprintln!("[Nova] Graph API authenticated successfully.");

                    let auth = Self {
                        access_token: access_token.clone(),
                        refresh_token: token_data.refresh_token,
                        expires_at_unix,
                        client_id: client_id.to_string(),
                        tenant_id: tenant_id.to_string(),
                        http,
                        cache_path: cache_path.to_path_buf(),
                    };
                    auth.save(cache_path)?;
                    return Ok(auth);
                }
                Some("authorization_pending") | Some("slow_down") => continue,
                Some("authorization_declined") => {
                    anyhow::bail!("Authentication declined by user.");
                }
                Some("expired_token") => {
                    anyhow::bail!(
                        "Device code expired. Run the tool again to retry."
                    );
                }
                Some(err) => {
                    anyhow::bail!("Authentication error: {err}");
                }
            }
        }
    }
}

// ── Mail Response Types ───────────────────────────────────────────────

/// A single mail message from the Graph messages endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailMessage {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<EmailAddressWrapper>,
    pub received_date_time: Option<String>,
    pub is_read: Option<bool>,
    pub has_attachments: Option<bool>,
    pub body_preview: Option<String>,
}

/// Wrapper around an email address object from Graph API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddressWrapper {
    pub email_address: Option<EmailAddress>,
}

/// An email address with display name and address.
#[derive(Debug, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

/// Graph API list response for mail messages.
#[derive(Debug, Deserialize)]
pub struct MailListResponse {
    pub value: Vec<MailMessage>,
}

/// Mail folder item from the mailFolders endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MailFolder {
    pub id: String,
    pub display_name: String,
}

/// Graph API list response for mail folders.
#[derive(Debug, Deserialize)]
struct MailFolderListResponse {
    pub value: Vec<MailFolder>,
}

// ── Calendar Response Types ───────────────────────────────────────────

/// A single calendar event from the Graph calendarView endpoint.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub subject: Option<String>,
    pub start: Option<DateTimeTimeZone>,
    pub end: Option<DateTimeTimeZone>,
    pub organizer: Option<CalendarOrganizer>,
    pub attendees: Option<Vec<CalendarAttendee>>,
    pub location: Option<CalendarLocation>,
    pub is_all_day: Option<bool>,
    pub is_cancelled: Option<bool>,
    pub body_preview: Option<String>,
}

/// Date-time with timezone from Graph API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: Option<String>,
    pub time_zone: Option<String>,
}

/// Calendar event organizer.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarOrganizer {
    pub email_address: Option<EmailAddress>,
}

/// Calendar event attendee.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarAttendee {
    pub email_address: Option<EmailAddress>,
}

/// Calendar event location.
#[derive(Debug, Deserialize)]
pub struct CalendarLocation {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

/// Graph API list response for calendar events.
#[derive(Debug, Deserialize)]
pub struct CalendarListResponse {
    pub value: Vec<CalendarEvent>,
}

// ── Single Email Response Types ───────────────────────────────────────

/// Full message body from Graph API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub content_type: Option<String>,
    pub content: Option<String>,
}

/// Full email message (for read_email).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullMailMessage {
    pub subject: Option<String>,
    pub from: Option<EmailAddressWrapper>,
    pub to_recipients: Option<Vec<EmailAddressWrapper>>,
    pub cc_recipients: Option<Vec<EmailAddressWrapper>>,
    pub received_date_time: Option<String>,
    pub body: Option<MessageBody>,
    pub has_attachments: Option<bool>,
    pub importance: Option<String>,
}

// ── HTTP Helper ───────────────────────────────────────────────────────

/// Perform an authenticated GET request, handling 401 (token refresh) and 429 (rate limit).
///
/// On 401: refreshes the token once and retries.
/// On 429: sleeps for `Retry-After` seconds (default 30) and retries once.
/// Other non-2xx responses: returns an error.
async fn get_json(auth: &mut GraphUserAuth, url: &str) -> Result<Value> {
    let token = auth.get_token().await?;

    let resp = auth
        .http
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await?;

    let status = resp.status();

    if status.as_u16() == 429 {
        // Rate limited — read Retry-After header and sleep
        let retry_after = resp
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        tokio::time::sleep(Duration::from_secs(retry_after)).await;

        // Retry once after backoff
        let token = auth.get_token().await?;
        let retry_resp = auth
            .http
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?;

        let retry_status = retry_resp.status();
        if !retry_status.is_success() {
            let body = retry_resp.text().await.unwrap_or_default();
            anyhow::bail!("Graph API error after 429 retry ({retry_status}): {body}");
        }
        return Ok(retry_resp.json().await?);
    }

    if status.as_u16() == 401 {
        // Token rejected — force a refresh and retry once
        let _ = auth.get_token().await; // May fail if no refresh token
        let token = auth.get_token().await?;
        let retry_resp = auth
            .http
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?;

        let retry_status = retry_resp.status();
        if !retry_status.is_success() {
            let body = retry_resp.text().await.unwrap_or_default();
            anyhow::bail!("Graph API error after 401 retry ({retry_status}): {body}");
        }
        return Ok(retry_resp.json().await?);
    }

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Graph API error ({status}): {body}");
    }

    Ok(resp.json().await?)
}

// ── outlook_inbox ─────────────────────────────────────────────────────

/// Fetch and format recent messages from a mail folder.
///
/// `folder` defaults to `Inbox`. Custom folder names are resolved case-insensitively
/// via the `mailFolders` endpoint.
pub async fn outlook_inbox(
    auth: &mut GraphUserAuth,
    folder: Option<&str>,
    count: u32,
    unread_only: bool,
) -> Result<String> {
    let folder_name = folder.unwrap_or("Inbox");

    // Resolve folder path — use wellKnownName for Inbox, otherwise look up by name
    let folder_path = if folder_name.eq_ignore_ascii_case("inbox") {
        "Inbox".to_string()
    } else {
        // Fetch up to 25 mail folders and match case-insensitively
        let folders_url = format!("{GRAPH_BASE}/me/mailFolders?$top=25");
        let folders_json = get_json(auth, &folders_url).await?;
        let folders: MailFolderListResponse = serde_json::from_value(folders_json)
            .map_err(|e| anyhow!("Failed to parse mail folders: {e}"))?;

        let matched = folders
            .value
            .into_iter()
            .find(|f| f.display_name.eq_ignore_ascii_case(folder_name));

        match matched {
            Some(f) => format!("mailFolders/{}/", f.id),
            None => anyhow::bail!("Mail folder not found: {folder_name}"),
        }
    };

    let folder_segment = if folder_path == "Inbox" {
        "mailFolders/Inbox/".to_string()
    } else {
        folder_path
    };

    let mut url = format!(
        "{GRAPH_BASE}/me/{folder_segment}messages\
         ?$select=id,subject,from,receivedDateTime,isRead,hasAttachments,bodyPreview\
         &$orderby=receivedDateTime desc\
         &$top={count}"
    );

    if unread_only {
        url.push_str("&$filter=isRead eq false");
    }

    let data = get_json(auth, &url).await?;
    let messages: MailListResponse =
        serde_json::from_value(data).map_err(|e| anyhow!("Failed to parse messages: {e}"))?;

    if messages.value.is_empty() {
        return Ok(format!("No messages in {folder_name}."));
    }

    let mut lines: Vec<String> = Vec::new();
    for (i, msg) in messages.value.iter().enumerate() {
        let subject = msg.subject.as_deref().unwrap_or("(no subject)");
        let from_display = format_email_address(msg.from.as_ref());
        let received = msg
            .received_date_time
            .as_deref()
            .map(super::relative_time)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "?".to_string());
        let attachment_mark = if msg.has_attachments.unwrap_or(false) {
            " [attachment]"
        } else {
            ""
        };
        let preview = msg
            .body_preview
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect::<String>();

        lines.push(format!("[{}] {}", i + 1, subject));
        lines.push(format!(
            "    From: {} · {}{}", from_display, received, attachment_mark
        ));
        if !preview.is_empty() {
            lines.push(format!("    {preview}"));
        }
    }

    Ok(lines.join("\n"))
}

// ── outlook_calendar ──────────────────────────────────────────────────

/// Fetch and format upcoming calendar events.
///
/// Groups events under day headers when `days_ahead > 1`.
pub async fn outlook_calendar(
    auth: &mut GraphUserAuth,
    days_ahead: u32,
    max_events: u32,
) -> Result<String> {
    let days_ahead = days_ahead.clamp(1, 14);
    let max_events = max_events.clamp(1, 25);

    let now = chrono::Utc::now();
    let end = now + chrono::Duration::days(days_ahead as i64);

    // calendarView requires RFC3339 format without sub-seconds
    let start_str = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end_str = end.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let url = format!(
        "{GRAPH_BASE}/me/calendarView\
         ?startDateTime={start_str}\
         &endDateTime={end_str}\
         &$select=subject,start,end,organizer,attendees,location,isAllDay,isCancelled,bodyPreview\
         &$top={max_events}\
         &$orderby=start/dateTime"
    );

    let data = get_json(auth, &url).await?;
    let events: CalendarListResponse =
        serde_json::from_value(data).map_err(|e| anyhow!("Failed to parse calendar events: {e}"))?;

    if events.value.is_empty() {
        let day_word = if days_ahead == 1 { "today" } else { &format!("the next {days_ahead} days") };
        return Ok(format!("No calendar events for {day_word}."));
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current_day: Option<String> = None;

    for event in &events.value {
        let is_all_day = event.is_all_day.unwrap_or(false);
        let is_cancelled = event.is_cancelled.unwrap_or(false);
        let subject = event.subject.as_deref().unwrap_or("(no subject)");

        // Determine time label
        let time_label = if is_all_day {
            "[All Day]".to_string()
        } else {
            let start_time = event
                .start
                .as_ref()
                .and_then(|s| s.date_time.as_deref())
                .map(parse_time_hhmm)
                .unwrap_or_else(|| "?".to_string());
            let end_time = event
                .end
                .as_ref()
                .and_then(|e| e.date_time.as_deref())
                .map(parse_time_hhmm)
                .unwrap_or_else(|| "?".to_string());
            format!("[{start_time}-{end_time}]")
        };

        // Group by day when days_ahead > 1
        if days_ahead > 1 {
            let event_day = event
                .start
                .as_ref()
                .and_then(|s| s.date_time.as_deref())
                .map(format_day_header)
                .unwrap_or_else(|| "Unknown Day".to_string());

            if current_day.as_deref() != Some(&event_day) {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.push(event_day.clone());
                current_day = Some(event_day);
            }
        }

        let cancelled_prefix = if is_cancelled { "[Cancelled] " } else { "" };
        lines.push(format!("{time_label} {cancelled_prefix}{subject}"));

        // Secondary line: organizer, attendee count, location
        let organizer = event
            .organizer
            .as_ref()
            .and_then(|o| o.email_address.as_ref())
            .and_then(|e| e.name.as_deref())
            .unwrap_or("?");
        let attendee_count = event
            .attendees
            .as_ref()
            .map(|a| a.len())
            .unwrap_or(0);
        let location = event
            .location
            .as_ref()
            .and_then(|l| l.display_name.as_deref())
            .filter(|l| !l.is_empty());

        let mut meta = format!("  {} · {} attendees", organizer, attendee_count);
        if let Some(loc) = location {
            meta.push_str(&format!(" · {loc}"));
        }
        lines.push(meta);
    }

    Ok(lines.join("\n"))
}

// ── outlook_read_email ────────────────────────────────────────────────

/// Fetch and format a single email by message ID.
///
/// Returns subject, headers, and a body with HTML stripped and whitespace
/// normalised, truncated to 4000 characters.
pub async fn outlook_read_email(auth: &mut GraphUserAuth, message_id: &str) -> Result<String> {
    let url = format!(
        "{GRAPH_BASE}/me/messages/{message_id}\
         ?$select=subject,from,toRecipients,ccRecipients,receivedDateTime,body,hasAttachments,importance"
    );

    let data = get_json(auth, &url).await?;

    // Check for 404 (Graph returns error in body when using get_json)
    // get_json already bails on non-2xx, but we catch message-specific errors here
    let msg: FullMailMessage = serde_json::from_value(data)
        .map_err(|e| anyhow!("Failed to parse message: {e}"))?;

    let subject = msg.subject.as_deref().unwrap_or("(no subject)");
    let from = format_email_address(msg.from.as_ref());

    let to_list = msg
        .to_recipients
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|w| format_email_address(Some(w)))
        .collect::<Vec<_>>()
        .join(", ");

    let date = msg
        .received_date_time
        .as_deref()
        .map(format_date_long)
        .unwrap_or_else(|| "?".to_string());

    let body_raw = msg
        .body
        .as_ref()
        .and_then(|b| b.content.as_deref())
        .unwrap_or("");

    let body_stripped = strip_html(body_raw);
    let body_truncated: String = body_stripped.chars().take(4000).collect();

    let mut output = format!(
        "Subject: {subject}\nFrom: {from}\nTo: {to_list}\nDate: {date}\n---\n{body_truncated}"
    );

    // Append CC if present and non-empty
    if let Some(cc) = &msg.cc_recipients {
        if !cc.is_empty() {
            let cc_list = cc
                .iter()
                .map(|w| format_email_address(Some(w)))
                .collect::<Vec<_>>()
                .join(", ");
            // Insert CC line after To line
            output = output.replacen(
                &format!("To: {to_list}"),
                &format!("To: {to_list}\nCC: {cc_list}"),
                1,
            );
        }
    }

    Ok(output)
}

// ── strip_html ────────────────────────────────────────────────────────

/// Strip HTML tags from a string and normalise whitespace.
///
/// Uses a simple `<[^>]+>` pattern — sufficient for email bodies.
/// Note: `teams.rs` in `nv-daemon` has the same helper.
pub fn strip_html(html: &str) -> String {
    // Replace block-level tags with newlines before stripping
    let with_newlines = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n");

    // Remove remaining HTML tags
    let mut result = String::with_capacity(with_newlines.len());
    let mut in_tag = false;
    for ch in with_newlines.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    // Decode common HTML entities
    let result = result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Normalise whitespace: collapse multiple consecutive blank lines into one
    let mut output = String::with_capacity(result.len());
    let mut consecutive_blanks = 0u32;

    for line in result.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks == 1 {
                output.push('\n');
            }
        } else {
            consecutive_blanks = 0;
            output.push_str(trimmed);
            output.push('\n');
        }
    }

    output.trim().to_string()
}

// ── Formatting Helpers ────────────────────────────────────────────────

/// Format an `EmailAddressWrapper` as `"Name <addr>"` or just `"addr"`.
fn format_email_address(wrapper: Option<&EmailAddressWrapper>) -> String {
    let ea = match wrapper.and_then(|w| w.email_address.as_ref()) {
        Some(ea) => ea,
        None => return "?".to_string(),
    };
    match (&ea.name, &ea.address) {
        (Some(name), Some(addr)) if !name.is_empty() => format!("{name} <{addr}>"),
        (None, Some(addr)) | (Some(_), Some(addr)) => addr.clone(),
        _ => "?".to_string(),
    }
}

/// Parse an ISO 8601 date-time string to `"HH:MM"`.
fn parse_time_hhmm(dt: &str) -> String {
    // Graph returns "2026-03-26T14:30:00.0000000" (no Z, UTC implied for calendarView)
    dt.get(11..16).unwrap_or("?").to_string()
}

/// Format an ISO 8601 date-time as `"Monday, Mar 25"`.
fn format_day_header(dt: &str) -> String {
    // Extract date portion (YYYY-MM-DD)
    let date_part = match dt.get(..10) {
        Some(d) => d,
        None => return "?".to_string(),
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    let year: i32 = parts[0].parse().unwrap_or(0);
    let month: u32 = parts[1].parse().unwrap_or(0);
    let day: u32 = parts[2].parse().unwrap_or(0);

    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

    let month_name = if (1..=12).contains(&month) {
        MONTHS[(month - 1) as usize]
    } else {
        "?"
    };

    // Compute day of week (Zeller's congruence, simplified)
    let dow = day_of_week(year, month, day);
    let day_name = DAYS[dow % 7];

    format!("{day_name}, {month_name} {day}")
}

/// Compute day of week index (0 = Sunday) using Tomohiko Sakamoto's algorithm.
fn day_of_week(y: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let result = (y + y / 4 - y / 100 + y / 400 + T[(m as usize) - 1] + d as i32) % 7;
    result.rem_euclid(7) as usize
}

/// Format a long date string like `"March 25, 2026 at 14:32"`.
fn format_date_long(dt: &str) -> String {
    if dt.len() < 16 {
        return dt.to_string();
    }
    let date_part = match dt.get(..10) {
        Some(d) => d,
        None => return dt.to_string(),
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return dt.to_string();
    }
    let year = parts[0];
    let month: usize = parts[1].parse().unwrap_or(0);
    let day: u32 = parts[2].parse().unwrap_or(0);
    let time = dt.get(11..16).unwrap_or("?");

    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let month_name = if (1..=12).contains(&month) {
        MONTHS[month - 1]
    } else {
        "?"
    };

    format!("{month_name} {day}, {year} at {time}")
}

// ── Utility ───────────────────────────────────────────────────────────

/// Return current Unix timestamp in seconds.
fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Resolve the graph user token cache path from env or default.
pub fn graph_user_token_path() -> PathBuf {
    if let Ok(path) = std::env::var("NV_GRAPH_USER_TOKEN_PATH") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("nv")
        .join("graph-user-token.json")
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn outlook_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "outlook_inbox".into(),
            description: "Read recent emails from an Outlook mail folder via Microsoft Graph. \
                Returns a formatted list of messages with sender, subject, timestamp, and preview. \
                Requires MS_GRAPH_CLIENT_ID and MS_GRAPH_TENANT_ID env vars; prompts for \
                device-code authentication on first use."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Mail folder to read (default: Inbox). \
                            Use folder display name to read other folders."
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of messages to return (default: 10, max: 25).",
                        "minimum": 1,
                        "maximum": 25,
                        "default": 10
                    },
                    "unread_only": {
                        "type": "boolean",
                        "description": "If true, only return unread messages (default: false).",
                        "default": false
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "outlook_calendar".into(),
            description: "Read upcoming calendar events from Outlook via Microsoft Graph. \
                Returns events with time, subject, organizer, attendee count, and location. \
                Requires MS_GRAPH_CLIENT_ID and MS_GRAPH_TENANT_ID env vars; prompts for \
                device-code authentication on first use."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "days_ahead": {
                        "type": "integer",
                        "description": "How many days ahead to fetch events (default: 1, max: 14).",
                        "minimum": 1,
                        "maximum": 14,
                        "default": 1
                    },
                    "max_events": {
                        "type": "integer",
                        "description": "Maximum events to return (default: 10, max: 25).",
                        "minimum": 1,
                        "maximum": 25,
                        "default": 10
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "outlook_read_email".into(),
            description: "Read the full body of a single Outlook email by message ID. \
                Returns subject, from, to, date, and body text with HTML stripped. \
                Use outlook_inbox to get message IDs. \
                Requires MS_GRAPH_CLIENT_ID and MS_GRAPH_TENANT_ID env vars."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message_id": {
                        "type": "string",
                        "description": "Microsoft Graph message ID (from outlook_inbox output)."
                    }
                },
                "required": ["message_id"]
            }),
        },
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tool Definitions ──────────────────────────────────────────────

    #[test]
    fn outlook_tool_definitions_count() {
        let defs = outlook_tool_definitions();
        assert_eq!(defs.len(), 3);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"outlook_inbox"));
        assert!(names.contains(&"outlook_calendar"));
        assert!(names.contains(&"outlook_read_email"));
    }

    #[test]
    fn outlook_tool_definitions_schemas_valid() {
        let defs = outlook_tool_definitions();
        for tool in &defs {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn outlook_read_email_has_required_message_id() {
        let defs = outlook_tool_definitions();
        let read_email = defs.iter().find(|d| d.name == "outlook_read_email").unwrap();
        let required = read_email.input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "message_id");
    }

    #[test]
    fn outlook_inbox_and_calendar_have_no_required_fields() {
        let defs = outlook_tool_definitions();
        for name in &["outlook_inbox", "outlook_calendar"] {
            let tool = defs.iter().find(|d| d.name == *name).unwrap();
            let required = tool.input_schema["required"].as_array().unwrap();
            assert!(required.is_empty(), "{name} should have no required fields");
        }
    }

    // ── strip_html ────────────────────────────────────────────────────

    #[test]
    fn strip_html_basic_tags() {
        let input = "<p>Hello <b>world</b></p>";
        let result = strip_html(input);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn strip_html_empty_string() {
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn strip_html_plain_text_unchanged() {
        let input = "Hello world";
        assert_eq!(strip_html(input), "Hello world");
    }

    #[test]
    fn strip_html_entities_decoded() {
        let input = "AT&amp;T &lt;email&gt; &quot;quoted&quot;";
        let result = strip_html(input);
        assert!(result.contains("AT&T"));
        assert!(result.contains("<email>"));
        assert!(result.contains("\"quoted\""));
    }

    #[test]
    fn strip_html_br_becomes_newline() {
        let input = "Line1<br/>Line2";
        let result = strip_html(input);
        assert!(result.contains("Line1"));
        assert!(result.contains("Line2"));
    }

    #[test]
    fn strip_html_whitespace_normalised() {
        let input = "<p>First</p>\n\n\n\n<p>Second</p>";
        let result = strip_html(input);
        assert!(result.contains("First"));
        assert!(result.contains("Second"));
        // Should not have more than 2 consecutive newlines
        assert!(!result.contains("\n\n\n"));
    }

    // ── Formatting helpers ────────────────────────────────────────────

    #[test]
    fn parse_time_hhmm_standard() {
        assert_eq!(parse_time_hhmm("2026-03-26T14:30:00.0000000"), "14:30");
    }

    #[test]
    fn parse_time_hhmm_short_string() {
        assert_eq!(parse_time_hhmm("2026"), "?");
    }

    #[test]
    fn format_day_header_known_date() {
        // 2026-03-26 is a Thursday
        let result = format_day_header("2026-03-26T14:30:00");
        assert!(result.contains("Mar"));
        assert!(result.contains("26"));
        assert!(result.contains("Thursday"));
    }

    #[test]
    fn format_date_long_standard() {
        let result = format_date_long("2026-03-26T14:32:00Z");
        assert!(result.contains("March"));
        assert!(result.contains("26"));
        assert!(result.contains("2026"));
        assert!(result.contains("14:32"));
    }

    #[test]
    fn format_email_address_with_name() {
        let wrapper = EmailAddressWrapper {
            email_address: Some(EmailAddress {
                name: Some("Leo Acosta".to_string()),
                address: Some("leo@example.com".to_string()),
            }),
        };
        let result = format_email_address(Some(&wrapper));
        assert_eq!(result, "Leo Acosta <leo@example.com>");
    }

    #[test]
    fn format_email_address_no_name() {
        let wrapper = EmailAddressWrapper {
            email_address: Some(EmailAddress {
                name: None,
                address: Some("leo@example.com".to_string()),
            }),
        };
        let result = format_email_address(Some(&wrapper));
        assert_eq!(result, "leo@example.com");
    }

    #[test]
    fn format_email_address_none() {
        let result = format_email_address(None);
        assert_eq!(result, "?");
    }

    // ── Token Cache ───────────────────────────────────────────────────

    #[test]
    fn graph_user_token_path_default() {
        let path = graph_user_token_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains(".config/nv/graph-user-token.json")
                || path_str.contains("NV_GRAPH_USER_TOKEN_PATH")
        );
    }

    #[test]
    fn graph_user_token_path_env_override() {
        std::env::set_var("NV_GRAPH_USER_TOKEN_PATH", "/tmp/test-token.json");
        let path = graph_user_token_path();
        assert_eq!(path.to_string_lossy(), "/tmp/test-token.json");
        std::env::remove_var("NV_GRAPH_USER_TOKEN_PATH");
    }

    #[test]
    fn from_cache_missing_file_returns_none() {
        let result = GraphUserAuth::from_cache(Path::new("/nonexistent/path/token.json"));
        assert!(result.is_none());
    }

    #[test]
    fn from_cache_expired_no_refresh_returns_none() {
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/nv-outlook-test-{id}.json"));

        let cache = serde_json::json!({
            "access_token": "expired-token",
            "refresh_token": null,
            "expires_at_unix": 1000u64,
            "client_id": "c",
            "tenant_id": "t"
        });
        std::fs::write(&path, serde_json::to_string(&cache).unwrap()).unwrap();

        let result = GraphUserAuth::from_cache(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_none());
    }

    #[test]
    fn from_cache_valid_token_loads() {
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/nv-outlook-test-{id}.json"));

        let future_unix = now_unix_secs() + 3600;
        let cache = serde_json::json!({
            "access_token": "valid-token",
            "refresh_token": "refresh-123",
            "expires_at_unix": future_unix,
            "client_id": "client-id",
            "tenant_id": "tenant-id"
        });
        std::fs::write(&path, serde_json::to_string(&cache).unwrap()).unwrap();

        let auth = GraphUserAuth::from_cache(&path);
        let _ = std::fs::remove_file(&path);
        let auth = auth.unwrap();
        assert_eq!(auth.access_token, "valid-token");
        assert_eq!(auth.client_id, "client-id");
        assert_eq!(auth.tenant_id, "tenant-id");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/nv-outlook-test-{id}.json"));

        let future_unix = now_unix_secs() + 3600;
        let auth = GraphUserAuth {
            access_token: "test-access".to_string(),
            refresh_token: Some("test-refresh".to_string()),
            expires_at_unix: future_unix,
            client_id: "client-123".to_string(),
            tenant_id: "tenant-456".to_string(),
            http: reqwest::Client::new(),
            cache_path: path.clone(),
        };

        auth.save(&path).unwrap();
        assert!(path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        let loaded = GraphUserAuth::from_cache(&path);
        let _ = std::fs::remove_file(&path);
        let loaded = loaded.unwrap();
        assert_eq!(loaded.access_token, "test-access");
        assert_eq!(loaded.refresh_token.as_deref(), Some("test-refresh"));
        assert_eq!(loaded.client_id, "client-123");
        assert_eq!(loaded.tenant_id, "tenant-456");
    }

    #[tokio::test]
    async fn get_token_returns_cached_when_valid() {
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/nv-outlook-test-{id}.json"));

        let future_unix = now_unix_secs() + 3600;
        let mut auth = GraphUserAuth {
            access_token: "cached-token".to_string(),
            refresh_token: None,
            expires_at_unix: future_unix,
            client_id: "c".to_string(),
            tenant_id: "t".to_string(),
            http: reqwest::Client::new(),
            cache_path: path,
        };

        let token = auth.get_token().await.unwrap();
        assert_eq!(token, "cached-token");
    }

    #[tokio::test]
    async fn get_token_errors_when_expired_no_refresh() {
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from(format!("/tmp/nv-outlook-test-{id}.json"));

        let mut auth = GraphUserAuth {
            access_token: "expired-token".to_string(),
            refresh_token: None,
            expires_at_unix: 1000, // very old
            client_id: "c".to_string(),
            tenant_id: "t".to_string(),
            http: reqwest::Client::new(),
            cache_path: path,
        };

        let result = auth.get_token().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("device code"));
    }

    // ── Integration test (behind feature flag) ────────────────────────

    #[cfg(feature = "integration")]
    #[test]
    fn stateless_tool_definitions_includes_outlook_tools() {
        use crate::dispatch::stateless_tool_definitions;
        let defs = stateless_tool_definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"outlook_inbox"));
        assert!(names.contains(&"outlook_calendar"));
        assert!(names.contains(&"outlook_read_email"));
    }
}
