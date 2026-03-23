//! Google Calendar API v3 read-only tools.
//!
//! Provides three tools that query Google Calendar via REST:
//! - `calendar_today` — events for today
//! - `calendar_upcoming` — events for the next N days (default 7, max 30)
//! - `calendar_next` — the single next upcoming event
//!
//! Auth: base64-encoded service account JSON key from `GOOGLE_CALENDAR_CREDENTIALS`
//! env var. Access token is fetched via JWT assertion (RS256) and cached in memory
//! with expiry. On 401, the token is refreshed automatically.
//!
//! All times are returned in the user's local timezone for display, but the API
//! receives RFC 3339 UTC timestamps.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDate, Timelike, Utc};
use serde::Deserialize;
use tokio::sync::Mutex;

/// Timeout for all Google Calendar API calls.
const CALENDAR_TIMEOUT: Duration = Duration::from_secs(15);

/// Base URL for the Calendar API.
const CALENDAR_BASE: &str = "https://www.googleapis.com/calendar/v3";

/// OAuth2 token endpoint for service accounts.
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// Calendar API scope (read-only).
const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";

// ── Response Types ───────────────────────────────────────────────────

/// An event date/time from the API (may be date-only for all-day events).
#[derive(Debug, Clone, Deserialize)]
pub struct EventDateTime {
    /// RFC 3339 datetime (for timed events).
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    /// Date string YYYY-MM-DD (for all-day events).
    pub date: Option<String>,
}

/// An event attendee.
#[derive(Debug, Clone, Deserialize)]
pub struct Attendee {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "self", default)]
    pub is_self: bool,
}

/// A conference entry point (for meeting links).
#[derive(Debug, Clone, Deserialize)]
pub struct EntryPoint {
    #[serde(rename = "entryPointType")]
    pub entry_point_type: String,
    pub uri: Option<String>,
}

/// Conference data attached to an event.
#[derive(Debug, Clone, Deserialize)]
pub struct ConferenceData {
    #[serde(rename = "entryPoints", default)]
    pub entry_points: Vec<EntryPoint>,
}

/// A single calendar event.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub summary: Option<String>,
    pub start: Option<EventDateTime>,
    pub end: Option<EventDateTime>,
    pub location: Option<String>,
    #[serde(rename = "attendees", default)]
    pub attendees: Vec<Attendee>,
    #[serde(rename = "conferenceData")]
    pub conference_data: Option<ConferenceData>,
    pub status: Option<String>,
}

/// Top-level events list response.
#[derive(Debug, Deserialize)]
pub struct EventList {
    pub items: Option<Vec<Event>>,
}

// ── Token Cache ──────────────────────────────────────────────────────

/// Cached OAuth2 access token with expiry.
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

impl CachedToken {
    fn is_valid(&self) -> bool {
        // Consider valid if more than 30 seconds remain
        Utc::now() + chrono::Duration::seconds(30) < self.expires_at
    }
}

/// Service account credentials parsed from the base64-encoded JSON key.
#[derive(Debug, Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    #[allow(dead_code)]
    project_id: Option<String>,
}

/// OAuth2 token response from Google.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

// ── Token Refresh ────────────────────────────────────────────────────

/// Decode and parse service account credentials from base64-encoded JSON.
fn parse_credentials(encoded: &str) -> Result<ServiceAccountKey> {
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .context("failed to base64-decode GOOGLE_CALENDAR_CREDENTIALS")?;
    let key: ServiceAccountKey = serde_json::from_slice(&decoded)
        .context("failed to parse service account JSON key")?;
    Ok(key)
}

/// Build a JWT assertion for service account auth.
///
/// Uses RS256 signing via raw RSA-PKCS1 (without ring/rustls-pki-types).
/// Since we already have `base64` as a dependency, we build the JWT manually.
fn build_jwt(creds: &ServiceAccountKey) -> Result<String> {
    use base64::Engine as _;

    let now = Utc::now().timestamp();
    let exp = now + 3600;

    // Header
    let header = serde_json::json!({
        "alg": "RS256",
        "typ": "JWT"
    });
    let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&header)?);

    // Claim set
    let claims = serde_json::json!({
        "iss": creds.client_email,
        "scope": CALENDAR_SCOPE,
        "aud": TOKEN_ENDPOINT,
        "exp": exp,
        "iat": now,
    });
    let claims_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&claims)?);

    let signing_input = format!("{header_b64}.{claims_b64}");

    // Sign with RS256 using the private key
    let signature = sign_rs256(creds.private_key.as_bytes(), signing_input.as_bytes())?;
    let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&signature);

    Ok(format!("{signing_input}.{sig_b64}"))
}

/// Sign data with RS256 (PKCS#1 v1.5 + SHA-256) using a PEM private key.
///
/// Calls `openssl dgst` as a subprocess (openssl is universally available
/// on Linux/Mac). Uses UUID-named temp files in /tmp to avoid a tempfile crate dep.
fn sign_rs256(private_key_pem: &[u8], message: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;
    use std::process::Command;

    let key_str = std::str::from_utf8(private_key_pem)
        .context("private key is not valid UTF-8")?;

    // Use a UUID-based name to avoid collisions
    let id = uuid::Uuid::new_v4();
    let key_path = format!("/tmp/nv-cal-key-{id}.pem");
    let msg_path = format!("/tmp/nv-cal-msg-{id}.bin");

    // Write key file
    {
        let mut f = std::fs::File::create(&key_path)
            .context("failed to create temp key file")?;
        f.write_all(key_str.as_bytes())
            .context("failed to write private key")?;
    }

    // Write message file
    {
        let mut f = std::fs::File::create(&msg_path)
            .context("failed to create temp message file")?;
        f.write_all(message)
            .context("failed to write message")?;
    }

    let output = Command::new("openssl")
        .args(["dgst", "-sha256", "-sign", &key_path, &msg_path])
        .output()
        .context("failed to run openssl — ensure openssl is installed");

    // Always clean up temp files
    let _ = std::fs::remove_file(&key_path);
    let _ = std::fs::remove_file(&msg_path);

    let output = output?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("openssl signing failed: {stderr}"));
    }

    Ok(output.stdout)
}

/// Obtain a new access token via service account JWT assertion.
async fn fetch_access_token(creds: &ServiceAccountKey) -> Result<CachedToken> {
    let jwt = build_jwt(creds)?;

    let client = reqwest::Client::builder()
        .timeout(CALENDAR_TIMEOUT)
        .build()?;

    let resp = client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await
        .context("failed to send token request")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("token request failed ({status}): {body}"));
    }

    let token_resp: TokenResponse = resp.json().await.context("failed to parse token response")?;
    let expires_at = Utc::now() + chrono::Duration::seconds(token_resp.expires_in);

    Ok(CachedToken {
        access_token: token_resp.access_token,
        expires_at,
    })
}

// ── Calendar Client ──────────────────────────────────────────────────

/// Shared calendar client with token caching.
pub struct CalendarClient {
    credentials_b64: String,
    calendar_id: String,
    token_cache: Arc<Mutex<Option<CachedToken>>>,
}

impl CalendarClient {
    /// Create a new client from base64-encoded credentials and calendar ID.
    pub fn new(credentials_b64: impl Into<String>, calendar_id: impl Into<String>) -> Self {
        Self {
            credentials_b64: credentials_b64.into(),
            calendar_id: calendar_id.into(),
            token_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Get a valid access token, refreshing if expired.
    async fn access_token(&self) -> Result<String> {
        let mut cache = self.token_cache.lock().await;
        if let Some(ref tok) = *cache {
            if tok.is_valid() {
                return Ok(tok.access_token.clone());
            }
        }

        let creds = parse_credentials(&self.credentials_b64)?;
        let token = fetch_access_token(&creds).await?;
        let access = token.access_token.clone();
        *cache = Some(token);
        Ok(access)
    }

    /// Query the Calendar API events endpoint.
    async fn query_events(&self, params: &[(&str, &str)]) -> Result<Vec<Event>> {
        let token = self.access_token().await?;

        let client = reqwest::Client::builder()
            .timeout(CALENDAR_TIMEOUT)
            .build()?;

        let url = format!("{CALENDAR_BASE}/calendars/{}/events", self.calendar_id);

        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .query(params)
            .send()
            .await
            .context("calendar API request failed")?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        match status.as_u16() {
            200 => {}
            401 => {
                // Invalidate cache so next call refreshes
                *self.token_cache.lock().await = None;
                return Err(anyhow!("Calendar credentials invalid or expired (401). Check GOOGLE_CALENDAR_CREDENTIALS."));
            }
            403 => {
                return Err(anyhow!(
                    "Calendar access denied (403) — ensure the service account has read access to calendar '{}'.",
                    self.calendar_id
                ));
            }
            404 => {
                return Err(anyhow!(
                    "Calendar '{}' not found (404). Check calendar_id in config.",
                    self.calendar_id
                ));
            }
            429 => {
                return Err(anyhow!("Google Calendar rate limited (429). Try again later."));
            }
            _ => {
                return Err(anyhow!("Calendar API error ({status}): {body}"));
            }
        }

        let event_list: EventList =
            serde_json::from_str(&body).context("failed to parse Calendar API response")?;

        Ok(event_list.items.unwrap_or_default())
    }
}

// ── Formatting Helpers ───────────────────────────────────────────────

/// Extract a human-readable time string from an EventDateTime.
fn format_event_time(dt: &EventDateTime) -> String {
    if let Some(ref s) = dt.date_time {
        // Parse RFC 3339 and format as HH:MM
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(s) {
            return format!("{:02}:{:02}", parsed.hour(), parsed.minute());
        }
    }
    if let Some(ref d) = dt.date {
        return format!("{d} (all day)");
    }
    "?".to_string()
}

/// Parse an EventDateTime as a UTC DateTime for sorting.
fn event_start_utc(evt: &Event) -> DateTime<Utc> {
    if let Some(ref edt) = evt.start {
        if let Some(ref s) = edt.date_time {
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(s) {
                return parsed.with_timezone(&Utc);
            }
        }
        if let Some(ref d) = edt.date {
            if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                if let Some(ndt) = date.and_hms_opt(0, 0, 0) {
                    return DateTime::from_naive_utc_and_offset(ndt, Utc);
                }
            }
        }
    }
    Utc::now()
}

/// Extract a meeting link from an event.
fn extract_meeting_link(evt: &Event) -> Option<String> {
    if let Some(ref cd) = evt.conference_data {
        for ep in &cd.entry_points {
            if ep.entry_point_type == "video" {
                if let Some(ref uri) = ep.uri {
                    return Some(uri.clone());
                }
            }
        }
    }
    None
}

/// Format attendee names (excluding self).
fn format_attendees(attendees: &[Attendee]) -> Option<String> {
    let names: Vec<String> = attendees
        .iter()
        .filter(|a| !a.is_self)
        .filter_map(|a| {
            a.display_name
                .clone()
                .or_else(|| a.email.clone())
        })
        .collect();

    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

/// Format a single event as a mobile-friendly display string.
fn format_event(evt: &Event) -> String {
    let title = evt.summary.as_deref().unwrap_or("(no title)");
    let start = evt.start.as_ref().map(format_event_time).unwrap_or_default();
    let end = evt.end.as_ref().map(format_event_time).unwrap_or_default();

    let mut detail_parts: Vec<String> = vec![format!("{start}–{end}")];

    if let Some(link) = extract_meeting_link(evt) {
        detail_parts.push(link);
    } else if let Some(ref loc) = evt.location {
        if !loc.trim().is_empty() {
            detail_parts.push(loc.clone());
        }
    }

    if let Some(names) = format_attendees(&evt.attendees) {
        detail_parts.push(names);
    }

    format!(
        "📅 **{title}**\n   {}",
        detail_parts.join(" · ")
    )
}

/// Group events by date (YYYY-MM-DD).
fn group_by_date(events: &[Event]) -> Vec<(String, Vec<&Event>)> {
    let mut groups: Vec<(String, Vec<&Event>)> = Vec::new();

    for evt in events {
        let date_key = if let Some(ref edt) = evt.start {
            if let Some(ref s) = edt.date_time {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default()
            } else if let Some(ref d) = edt.date {
                d.clone()
            } else {
                continue;
            }
        } else {
            continue;
        };

        if let Some(group) = groups.iter_mut().find(|(k, _)| k == &date_key) {
            group.1.push(evt);
        } else {
            groups.push((date_key, vec![evt]));
        }
    }

    groups
}

// ── Public Tool Functions ────────────────────────────────────────────

/// Get today's events.
pub async fn calendar_today(client: &CalendarClient) -> Result<String> {
    let now = Utc::now();
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or(now);
    let day_end = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or(now);

    let time_min = day_start.to_rfc3339();
    let time_max = day_end.to_rfc3339();

    let params = [
        ("timeMin", time_min.as_str()),
        ("timeMax", time_max.as_str()),
        ("singleEvents", "true"),
        ("orderBy", "startTime"),
    ];

    let mut events = client.query_events(&params).await?;

    // Filter out cancelled events
    events.retain(|e| e.status.as_deref() != Some("cancelled"));

    if events.is_empty() {
        return Ok("No events scheduled for today.".to_string());
    }

    let mut lines = vec![format!("Today's schedule ({} event{}):", events.len(), if events.len() == 1 { "" } else { "s" })];
    for evt in &events {
        lines.push(format_event(evt));
    }

    Ok(lines.join("\n"))
}

/// Get events for the next N days (default 7, max 30).
pub async fn calendar_upcoming(client: &CalendarClient, days: Option<u32>) -> Result<String> {
    let days = days.unwrap_or(7).clamp(1, 30);
    let now = Utc::now();
    let end = now + chrono::Duration::days(days as i64);

    let time_min = now.to_rfc3339();
    let time_max = end.to_rfc3339();

    let params = [
        ("timeMin", time_min.as_str()),
        ("timeMax", time_max.as_str()),
        ("singleEvents", "true"),
        ("orderBy", "startTime"),
        ("maxResults", "100"),
    ];

    let mut events = client.query_events(&params).await?;

    // Filter out cancelled events
    events.retain(|e| e.status.as_deref() != Some("cancelled"));

    if events.is_empty() {
        return Ok(format!("No events in the next {days} day{}.", if days == 1 { "" } else { "s" }));
    }

    let groups = group_by_date(&events);
    let mut lines = vec![format!("Upcoming events — next {days} day{} ({} total):", if days == 1 { "" } else { "s" }, events.len())];

    for (date, day_events) in &groups {
        // Parse date for a friendly label
        let label = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .ok()
            .map(|d| d.format("%a %b %-d").to_string())
            .unwrap_or_else(|| date.clone());

        lines.push(format!("\n{label}:"));
        for evt in day_events {
            lines.push(format_event(evt));
        }
    }

    Ok(lines.join("\n"))
}

/// Get the next single upcoming event.
pub async fn calendar_next(client: &CalendarClient) -> Result<String> {
    let now = Utc::now();
    let time_min = now.to_rfc3339();

    let params = [
        ("timeMin", time_min.as_str()),
        ("singleEvents", "true"),
        ("orderBy", "startTime"),
        ("maxResults", "1"),
    ];

    let mut events = client.query_events(&params).await?;

    // Filter cancelled
    events.retain(|e| e.status.as_deref() != Some("cancelled"));

    match events.first() {
        None => Ok("No upcoming events.".to_string()),
        Some(evt) => {
            let mut lines = vec!["Next event:".to_string()];
            lines.push(format_event(evt));
            Ok(lines.join("\n"))
        }
    }
}

// ── Client Construction Helper ───────────────────────────────────────

/// Build a CalendarClient from env + config, or return a "not configured" error.
pub fn build_client(
    credentials_b64: Option<&str>,
    calendar_id: &str,
) -> Result<CalendarClient> {
    let creds = credentials_b64
        .ok_or_else(|| anyhow!("Calendar not configured: GOOGLE_CALENDAR_CREDENTIALS is not set."))?;

    Ok(CalendarClient::new(creds, calendar_id))
}

// ── Digest Helpers ───────────────────────────────────────────────────

/// Lightweight event summary for the daily digest.
#[derive(Debug, Clone)]
pub struct CalendarDigestEvent {
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub attendees_count: usize,
    pub has_meeting_link: bool,
}

/// Fetch today's events and map them to `CalendarDigestEvent` structs.
///
/// Returns an empty vec with no error if calendar is not configured.
pub async fn gather_today_for_digest(
    credentials_b64: Option<&str>,
    calendar_id: &str,
) -> Result<Vec<CalendarDigestEvent>> {
    let Some(creds) = credentials_b64 else {
        return Ok(Vec::new());
    };

    let client = CalendarClient::new(creds, calendar_id);
    let now = Utc::now();
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or(now);
    let day_end = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or(now);

    let time_min = day_start.to_rfc3339();
    let time_max = day_end.to_rfc3339();

    let params = [
        ("timeMin", time_min.as_str()),
        ("timeMax", time_max.as_str()),
        ("singleEvents", "true"),
        ("orderBy", "startTime"),
    ];

    let mut events = client.query_events(&params).await?;
    events.retain(|e| e.status.as_deref() != Some("cancelled"));

    // Sort by start time
    events.sort_by_key(event_start_utc);

    Ok(events
        .iter()
        .map(|evt| CalendarDigestEvent {
            title: evt.summary.clone().unwrap_or_else(|| "(no title)".to_string()),
            start_time: evt.start.as_ref().map(format_event_time).unwrap_or_default(),
            end_time: evt.end.as_ref().map(format_event_time).unwrap_or_default(),
            attendees_count: evt.attendees.iter().filter(|a| !a.is_self).count(),
            has_meeting_link: extract_meeting_link(evt).is_some(),
        })
        .collect())
}
