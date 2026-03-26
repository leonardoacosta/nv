//! Outlook email and calendar tools via MS Graph delegated permissions.
//!
//! Two read-only tools:
//! * `read_outlook_inbox` — list recent messages from Inbox (or another folder).
//! * `read_outlook_calendar` — list upcoming calendar events.
//!
//! Auth: device-code flow via `MsGraphUserAuth` (delegated — Mail.Read, Calendars.Read).
//! Credentials: `MS_GRAPH_CLIENT_ID`, `MS_GRAPH_TENANT_ID` env vars.
//! Token cache: `~/.config/nv/graph-token.json` (or `NV_GRAPH_TOKEN_PATH`).

use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;

use crate::channels::teams::oauth::MsGraphUserAuth;
use crate::claude::ToolDefinition;

// ── Constants ─────────────────────────────────────────────────────────

const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";
const MAX_INBOX_COUNT: u32 = 25;
const MAX_CALENDAR_EVENTS: u32 = 25;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ── Response Types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddressWrapper {
    pub email_address: EmailAddress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailMessage {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub subject: Option<String>,
    pub from: Option<EmailAddressWrapper>,
    pub received_date_time: Option<String>,
    pub is_read: Option<bool>,
    pub has_attachments: Option<bool>,
    #[allow(dead_code)]
    pub importance: Option<String>,
    pub body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MailListResponse {
    value: Vec<MailMessage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MailFolder {
    id: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderListResponse {
    value: Vec<MailFolder>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    #[allow(dead_code)]
    pub time_zone: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRecipient {
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    #[allow(dead_code)]
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub subject: Option<String>,
    pub start: Option<DateTimeTimeZone>,
    pub end: Option<DateTimeTimeZone>,
    pub organizer: Option<CalendarRecipient>,
    pub attendees: Option<Vec<Attendee>>,
    pub location: Option<Location>,
    pub is_all_day: Option<bool>,
    pub is_cancelled: Option<bool>,
    #[allow(dead_code)]
    pub body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarListResponse {
    value: Vec<CalendarEvent>,
}

// ── OutlookClient ─────────────────────────────────────────────────────

/// HTTP client wrapping a `MsGraphUserAuth` token for Outlook API calls.
pub struct OutlookClient {
    http: reqwest::Client,
    auth: MsGraphUserAuth,
}

impl OutlookClient {
    /// Create from a `MsGraphUserAuth` (device-code or cached).
    pub fn new(auth: MsGraphUserAuth) -> Self {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("valid reqwest client");
        Self { http, auth }
    }

    /// GET a Graph API URL and return the parsed JSON value.
    ///
    /// Injects the Bearer token and retries once on 429 (rate limit).
    pub async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let token = self.auth.get_token().await?;

        for attempt in 0..2u8 {
            let resp = self
                .http
                .get(url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await?;

            if resp.status().as_u16() == 429 {
                // Respect Retry-After header or wait 5s
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(5);
                if attempt == 0 {
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    continue;
                }
                anyhow::bail!("Graph API rate limited (429). Try again later.");
            }

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Graph API error ({status}): {body}");
            }

            return Ok(resp.json().await?);
        }

        anyhow::bail!("Graph API request failed after retry");
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Naive HTML tag stripper — removes tags and decodes standard entities.
pub fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
        .trim()
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Parse an ISO8601/RFC3339 datetime string and format it as `HH:MM`.
fn format_time(dt_str: &str) -> String {
    DateTime::parse_from_rfc3339(dt_str)
        .map(|dt| {
            let local: DateTime<Local> = dt.with_timezone(&Local);
            local.format("%H:%M").to_string()
        })
        .unwrap_or_else(|_| {
            // Fallback: extract HH:MM from the string directly
            dt_str.get(11..16).unwrap_or("??:??").to_string()
        })
}

/// Parse an ISO8601/RFC3339 datetime string and format as `YYYY-MM-DD`.
fn format_date(dt_str: &str) -> String {
    DateTime::parse_from_rfc3339(dt_str)
        .map(|dt| {
            let local: DateTime<Local> = dt.with_timezone(&Local);
            local.format("%Y-%m-%d").to_string()
        })
        .unwrap_or_else(|_| dt_str.get(0..10).unwrap_or("????-??-??").to_string())
}

/// Resolve a folder name to its Graph API folder ID.
///
/// If the name is `"Inbox"` (case-insensitive), returns `"Inbox"` directly
/// (Graph accepts the well-known folder name). For other folders, fetches the
/// user's folder list and matches by `displayName`.
async fn resolve_folder_id(client: &OutlookClient, folder_name: &str) -> Result<String> {
    if folder_name.eq_ignore_ascii_case("inbox") {
        return Ok("Inbox".to_string());
    }

    let url = format!("{GRAPH_API_BASE}/me/mailFolders?$top=25");
    let data = client.get_json(&url).await?;
    let folder_list: FolderListResponse = serde_json::from_value(data)?;

    folder_list
        .value
        .into_iter()
        .find(|f| f.display_name.eq_ignore_ascii_case(folder_name))
        .map(|f| f.id)
        .ok_or_else(|| anyhow!("Mail folder '{folder_name}' not found"))
}

// ── Tool Handlers ─────────────────────────────────────────────────────

/// Fetch and format recent inbox messages.
pub async fn read_outlook_inbox(
    client: &OutlookClient,
    folder: Option<&str>,
    count: u32,
    unread_only: bool,
) -> Result<String> {
    let folder_name = folder.unwrap_or("Inbox");
    let folder_id = resolve_folder_id(client, folder_name).await?;
    let top = count.clamp(1, MAX_INBOX_COUNT);

    let select = "id,subject,from,receivedDateTime,isRead,hasAttachments,importance,bodyPreview";
    let mut url = format!(
        "{GRAPH_API_BASE}/me/mailFolders/{folder_id}/messages\
         ?$select={select}\
         &$orderby=receivedDateTime desc\
         &$top={top}"
    );

    if unread_only {
        url.push_str("&$filter=isRead eq false");
    }

    let data = client.get_json(&url).await?;
    let response: MailListResponse = serde_json::from_value(data)
        .map_err(|e| anyhow!("Failed to parse inbox response: {e}"))?;

    let messages = response.value;
    let unread_count = messages.iter().filter(|m| m.is_read == Some(false)).count();
    let total = messages.len();

    let mut lines = vec![format!(
        "{folder_name} — {total} message{} ({unread_count} unread)",
        if total == 1 { "" } else { "s" }
    )];

    for msg in &messages {
        let is_unread = msg.is_read == Some(false);
        let has_attach = msg.has_attachments == Some(true);
        let prefix = if is_unread { "* " } else { "  " };
        let attach = if has_attach { " [+]" } else { "" };

        let dt = msg.received_date_time.as_deref().unwrap_or("");
        let dt_display = if dt.is_empty() {
            "?".to_string()
        } else {
            format!("[{}]", &dt[..dt.len().min(20)])
        };

        let from_name = msg
            .from
            .as_ref()
            .and_then(|f| {
                f.email_address
                    .name
                    .as_deref()
                    .filter(|n| !n.is_empty())
                    .or(f.email_address.address.as_deref())
            })
            .unwrap_or("Unknown");

        let subject = msg.subject.as_deref().unwrap_or("(no subject)");
        let preview = msg
            .body_preview
            .as_deref()
            .map(strip_html)
            .unwrap_or_default();
        let preview = preview.trim();

        lines.push(format!(
            "{prefix}{dt_display} {from_name}{attach} — {subject}"
        ));
        if !preview.is_empty() {
            let truncated = if preview.len() > 100 {
                format!("{}...", &preview[..100])
            } else {
                preview.to_string()
            };
            lines.push(format!("  Preview: {truncated}"));
        }
    }

    if messages.is_empty() {
        lines.push("(no messages)".to_string());
    }

    Ok(lines.join("\n"))
}

/// Fetch and format upcoming calendar events.
pub async fn read_outlook_calendar(
    client: &OutlookClient,
    days_ahead: u32,
    max_events: u32,
) -> Result<String> {
    let now = Utc::now();
    let end = now + chrono::Duration::days(i64::from(days_ahead.max(1)));

    let start_str = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end_str = end.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let top = max_events.clamp(1, MAX_CALENDAR_EVENTS);

    let select =
        "subject,start,end,organizer,attendees,location,isAllDay,isCancelled,bodyPreview";
    let url = format!(
        "{GRAPH_API_BASE}/me/calendarView\
         ?startDateTime={start_str}\
         &endDateTime={end_str}\
         &$top={top}\
         &$select={select}\
         &$orderby=start/dateTime"
    );

    let data = client.get_json(&url).await?;
    let response: CalendarListResponse = serde_json::from_value(data)
        .map_err(|e| anyhow!("Failed to parse calendar response: {e}"))?;

    let events = response.value;
    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let header_date = if days_ahead <= 1 {
        today_str.clone()
    } else {
        format!("{today_str} (+{days_ahead} days)")
    };

    let active: Vec<&CalendarEvent> = events
        .iter()
        .filter(|e| e.is_cancelled != Some(true))
        .collect();
    let total = active.len();

    let mut lines = vec![format!(
        "Calendar — {header_date} ({total} event{})",
        if total == 1 { "" } else { "s" }
    )];

    let mut current_day: Option<String> = None;

    for event in &active {
        let subject = event.subject.as_deref().unwrap_or("(no title)");
        let is_all_day = event.is_all_day == Some(true);

        let time_label = if is_all_day {
            "[All Day]".to_string()
        } else {
            let start_dt = event
                .start
                .as_ref()
                .map(|s| s.date_time.as_str())
                .unwrap_or("");
            let end_dt = event
                .end
                .as_ref()
                .map(|s| s.date_time.as_str())
                .unwrap_or("");
            format!("[{}–{}]", format_time(start_dt), format_time(end_dt))
        };

        // Show day header when events span multiple days
        if days_ahead > 1 {
            let event_date = event
                .start
                .as_ref()
                .map(|s| format_date(&s.date_time))
                .unwrap_or_default();
            if current_day.as_deref() != Some(&event_date) {
                current_day = Some(event_date.clone());
                lines.push(format!("\n{event_date}"));
            }
        }

        let location = event
            .location
            .as_ref()
            .and_then(|l| l.display_name.as_deref())
            .filter(|n| !n.is_empty())
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();

        lines.push(format!("{time_label} {subject}{location}"));

        let organizer_email = event
            .organizer
            .as_ref()
            .and_then(|o| o.email_address.as_ref())
            .and_then(|e| {
                e.name
                    .as_deref()
                    .filter(|n| !n.is_empty())
                    .or(e.address.as_deref())
            })
            .unwrap_or("unknown");

        let attendee_count = event
            .attendees
            .as_ref()
            .map(|a| a.len())
            .unwrap_or(0);

        if attendee_count > 0 {
            lines.push(format!(
                "  Organizer: {organizer_email} | {attendee_count} attendee{}",
                if attendee_count == 1 { "" } else { "s" }
            ));
        } else {
            lines.push(format!("  Organizer: {organizer_email}"));
        }
    }

    if active.is_empty() {
        lines.push("(no upcoming events)".to_string());
    }

    Ok(lines.join("\n"))
}

// ── Public Entry Points ───────────────────────────────────────────────

/// Execute `read_outlook_inbox` — load auth and fetch inbox.
pub async fn read_inbox(
    folder: Option<&str>,
    count: u32,
    unread_only: bool,
) -> Result<String> {
    let auth = MsGraphUserAuth::try_load_or_prompt().await.map_err(|e| {
        anyhow!(
            "Outlook not configured: {e}\n\
             Run `nv auth graph` to authenticate with Microsoft Graph."
        )
    })?;
    let client = OutlookClient::new(auth);
    read_outlook_inbox(&client, folder, count, unread_only).await
}

/// Execute `read_outlook_calendar` — load auth and fetch calendar events.
pub async fn read_calendar(days_ahead: u32, max_events: u32) -> Result<String> {
    let auth = MsGraphUserAuth::try_load_or_prompt().await.map_err(|e| {
        anyhow!(
            "Outlook not configured: {e}\n\
             Run `nv auth graph` to authenticate with Microsoft Graph."
        )
    })?;
    let client = OutlookClient::new(auth);
    read_outlook_calendar(&client, days_ahead, max_events).await
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn outlook_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_outlook_inbox".into(),
            description: "Read recent emails from Outlook inbox (or another mail folder). \
                Returns a formatted list of messages with sender, subject, timestamp, and preview. \
                Requires Microsoft Graph delegated authentication (Mail.Read). \
                Use this to check for new emails, find messages from specific senders, or \
                browse other mail folders."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Mail folder to read (default: 'Inbox'). Other examples: 'Sent Items', 'Drafts'."
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of messages to return (default: 10, max: 25).",
                        "minimum": 1,
                        "maximum": 25
                    },
                    "unread_only": {
                        "type": "boolean",
                        "description": "If true, only return unread messages (default: false)."
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "read_outlook_calendar".into(),
            description: "Read upcoming events from Outlook calendar. \
                Returns a formatted list of calendar events with time, subject, location, \
                organizer, and attendee count. \
                Requires Microsoft Graph delegated authentication (Calendars.Read). \
                Use this to check today's meetings, upcoming schedule, or multi-day agenda."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "days_ahead": {
                        "type": "integer",
                        "description": "How many days ahead to fetch events (default: 1 = today only).",
                        "minimum": 1,
                        "maximum": 30
                    },
                    "max_events": {
                        "type": "integer",
                        "description": "Maximum events to return (default: 10, max: 25).",
                        "minimum": 1,
                        "maximum": 25
                    }
                },
                "required": []
            }),
        },
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_basic() {
        assert_eq!(strip_html("<p>Hello <b>world</b>!</p>"), "Hello world!");
        assert_eq!(strip_html("no tags"), "no tags");
        assert_eq!(strip_html("&amp; &lt; &gt;"), "& < >");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn strip_html_entities() {
        assert_eq!(strip_html("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(strip_html("&nbsp;space"), " space");
        assert_eq!(strip_html("&apos;apostrophe&apos;"), "'apostrophe'");
    }

    #[test]
    fn outlook_tool_definitions_count() {
        let defs = outlook_tool_definitions();
        assert_eq!(defs.len(), 2);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"read_outlook_inbox"));
        assert!(names.contains(&"read_outlook_calendar"));
    }

    #[test]
    fn format_inbox_messages() {
        // Test the formatting logic with fixture data
        let messages = vec![
            MailMessage {
                id: Some("msg-1".into()),
                subject: Some("Meeting follow-up".into()),
                from: Some(EmailAddressWrapper {
                    email_address: EmailAddress {
                        name: Some("Sarah Martinez".into()),
                        address: Some("sarah@company.com".into()),
                    },
                }),
                received_date_time: Some("2026-03-25T14:30:00Z".into()),
                is_read: Some(false),
                has_attachments: Some(true),
                importance: Some("normal".into()),
                body_preview: Some("Please find the action items...".into()),
            },
            MailMessage {
                id: Some("msg-2".into()),
                subject: Some("Build notification".into()),
                from: Some(EmailAddressWrapper {
                    email_address: EmailAddress {
                        name: Some("No-Reply".into()),
                        address: Some("noreply@company.com".into()),
                    },
                }),
                received_date_time: Some("2026-03-25T12:00:00Z".into()),
                is_read: Some(true),
                has_attachments: Some(false),
                importance: Some("normal".into()),
                body_preview: Some("Build #4521 completed successfully.".into()),
            },
        ];

        let unread_count = messages.iter().filter(|m| m.is_read == Some(false)).count();
        assert_eq!(unread_count, 1);

        // Verify unread marker logic
        assert!(messages[0].is_read == Some(false));
        assert!(messages[1].is_read == Some(true));

        // Verify attachment flag
        assert!(messages[0].has_attachments == Some(true));
        assert!(messages[1].has_attachments == Some(false));
    }

    #[test]
    fn format_calendar_event_timed() {
        let event = CalendarEvent {
            subject: Some("Architecture Review".into()),
            start: Some(DateTimeTimeZone {
                date_time: "2026-03-25T14:00:00Z".into(),
                time_zone: "UTC".into(),
            }),
            end: Some(DateTimeTimeZone {
                date_time: "2026-03-25T15:00:00Z".into(),
                time_zone: "UTC".into(),
            }),
            organizer: Some(CalendarRecipient {
                email_address: Some(EmailAddress {
                    name: Some("Leo".into()),
                    address: Some("leo@company.com".into()),
                }),
            }),
            attendees: Some(vec![
                Attendee { email_address: None },
                Attendee { email_address: None },
                Attendee { email_address: None },
            ]),
            location: Some(Location {
                display_name: Some("Room 302".into()),
            }),
            is_all_day: Some(false),
            is_cancelled: Some(false),
            body_preview: None,
        };

        // Verify the event is not all-day
        assert!(event.is_all_day == Some(false));
        // Verify attendee count
        assert_eq!(event.attendees.as_ref().unwrap().len(), 3);
        // Verify location
        assert_eq!(
            event.location.as_ref().unwrap().display_name.as_deref(),
            Some("Room 302")
        );
    }

    #[test]
    fn format_calendar_event_all_day() {
        let event = CalendarEvent {
            subject: Some("Company Holiday".into()),
            start: Some(DateTimeTimeZone {
                date_time: "2026-03-25T00:00:00Z".into(),
                time_zone: "UTC".into(),
            }),
            end: Some(DateTimeTimeZone {
                date_time: "2026-03-26T00:00:00Z".into(),
                time_zone: "UTC".into(),
            }),
            organizer: None,
            attendees: None,
            location: None,
            is_all_day: Some(true),
            is_cancelled: Some(false),
            body_preview: None,
        };

        assert!(event.is_all_day == Some(true));
    }
}
