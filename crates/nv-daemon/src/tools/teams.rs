//! Microsoft Teams tools via MS Graph REST API.
//!
//! Provides four Claude-callable tools:
//! - `teams_channels` — list channels in a team
//! - `teams_messages` — read recent messages from a channel
//! - `teams_send`     — send a message to a channel (requires PendingAction confirmation)
//! - `teams_presence` — check a user's presence/availability status
//!
//! Auth: reuses `MsGraphAuth` from `channels/teams/oauth.rs`. Tenant ID resolution
//! order: `MS_GRAPH_TENANT_ID` env var > `[teams].tenant_id` in config > error.
//! Client ID and secret come from `MS_GRAPH_CLIENT_ID` / `MS_GRAPH_CLIENT_SECRET`.
//!
//! This module is separate from the channel adapter (inbound webhook relay).
//! The tools construct a standalone `TeamsClient` for outbound API calls.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use nv_core::config::{Secrets, TeamsConfig};

use crate::channels::teams::client::TeamsClient;
use crate::channels::teams::oauth::MsGraphAuth;
use crate::claude::ToolDefinition;

// ── Client Construction ───────────────────────────────────────────────

/// Build a standalone `TeamsClient` for tool use.
///
/// Resolves credentials from `secrets` and `teams_config`:
/// - Tenant ID: `MS_GRAPH_TENANT_ID` env var (via `secrets.ms_graph_tenant_id`) >
///   `[teams].tenant_id` in config
/// - Client ID: `MS_GRAPH_CLIENT_ID`
/// - Client Secret: `MS_GRAPH_CLIENT_SECRET`
///
/// Returns an error if any required credential is missing.
pub fn build_teams_client(
    secrets: &Secrets,
    teams_config: Option<&TeamsConfig>,
) -> Result<TeamsClient> {
    let tenant_id = secrets
        .ms_graph_tenant_id
        .as_deref()
        .or_else(|| teams_config.map(|c| c.tenant_id.as_str()))
        .ok_or_else(|| {
            anyhow!(
                "MS Graph tenant ID not configured. Set MS_GRAPH_TENANT_ID env var \
                or [teams].tenant_id in nv.toml."
            )
        })?;

    let client_id = secrets
        .ms_graph_client_id
        .as_deref()
        .ok_or_else(|| anyhow!("MS_GRAPH_CLIENT_ID not set"))?;

    let client_secret = secrets
        .ms_graph_client_secret
        .as_deref()
        .ok_or_else(|| anyhow!("MS_GRAPH_CLIENT_SECRET not set"))?;

    let auth = Arc::new(MsGraphAuth::new(tenant_id, client_id, client_secret));
    Ok(TeamsClient::new(auth))
}

// ── Tool Handlers ────────────────────────────────────────────────────

/// List channels in a Teams team.
///
/// Calls `TeamsClient::list_channels(team_id)` and formats the result as a
/// readable list with channel ID, display name, and description.
pub async fn teams_channels(client: &TeamsClient, team_id: &str) -> Result<String> {
    let channels = client.list_channels(team_id).await.map_err(|e| {
        if e.to_string().contains("401") {
            anyhow!("MS Graph auth invalid (401). Token may have expired — credentials may be wrong.")
        } else if e.to_string().contains("403") {
            anyhow!("Insufficient permissions to list channels (403). Ensure the Azure AD app has Channel.ReadBasic.All permission.")
        } else if e.to_string().contains("404") {
            anyhow!("Team '{}' not found (404).", team_id)
        } else {
            e
        }
    })?;

    if channels.is_empty() {
        return Ok(format!("No channels found in team `{team_id}`."));
    }

    let mut lines = vec![format!(
        "💬 **{}** — {} channel{}",
        team_id,
        channels.len(),
        if channels.len() == 1 { "" } else { "s" }
    )];
    for ch in &channels {
        let desc = ch
            .description
            .as_deref()
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        lines.push(format!("   • {} ({}){}", ch.display_name, ch.id, desc));
    }

    Ok(lines.join("\n"))
}

/// Get recent messages from a Teams channel.
///
/// Calls `TeamsClient::get_channel_messages(team_id, channel_id, 20)` and
/// formats each message with sender, timestamp, and a 200-character content preview.
pub async fn teams_messages(
    client: &TeamsClient,
    team_id: &str,
    channel_id: &str,
) -> Result<String> {
    let messages = client
        .get_channel_messages(team_id, channel_id, 20)
        .await?;

    if messages.is_empty() {
        return Ok(format!(
            "No messages found in channel `{channel_id}` of team `{team_id}`."
        ));
    }

    let mut lines = vec![format!(
        "💬 **{channel_id}** — {} message{}",
        messages.len(),
        if messages.len() == 1 { "" } else { "s" }
    )];

    for msg in &messages {
        let sender = msg
            .from
            .as_ref()
            .and_then(|f| f.user.as_ref())
            .and_then(|u| u.display_name.as_deref())
            .unwrap_or("unknown");

        let timestamp = msg.created_date_time.as_deref().unwrap_or("unknown time");

        // Strip HTML tags if content_type is html, then truncate to 200 chars
        let raw_content = if msg
            .body
            .content_type
            .as_deref()
            .map(|ct| ct.eq_ignore_ascii_case("html"))
            .unwrap_or(false)
        {
            strip_html(&msg.body.content)
        } else {
            msg.body.content.clone()
        };

        let preview = truncate_to_chars(&raw_content, 200);
        lines.push(format!("   [{timestamp}] {sender}: {preview}"));
    }

    Ok(lines.join("\n"))
}

/// Check a Teams user's presence status.
///
/// Calls `TeamsClient::get_user_presence(user)` and formats the result as
/// `"Name (user@domain.com): Available — InACall"`.
pub async fn teams_presence(client: &TeamsClient, user: &str) -> Result<String> {
    let presence = client.get_user_presence(user).await?;
    Ok(format!(
        "{user}: {} — {}",
        presence.availability, presence.activity
    ))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return the 4 Teams tool definitions for the Anthropic API.
pub fn teams_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "teams_channels".into(),
            description: "List channels in a Microsoft Teams team. Returns channel IDs, \
                display names, and descriptions. Use team_id parameter or falls back to \
                [teams].team_id config. Requires Channel.ReadBasic.All Azure AD permission."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "Teams team ID (GUID). Optional if [teams].team_id is configured."
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "teams_messages".into(),
            description: "Read recent messages from a Microsoft Teams channel. Returns the last \
                20 messages with sender name, timestamp, and content preview (truncated to 200 chars). \
                Requires ChannelMessage.Read.All Azure AD permission."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "Teams team ID (GUID). Optional if [teams].team_id is configured."
                    },
                    "channel_id": {
                        "type": "string",
                        "description": "Teams channel ID (GUID). Required."
                    }
                },
                "required": ["channel_id"]
            }),
        },
        ToolDefinition {
            name: "teams_send".into(),
            description: "Send a message to a Microsoft Teams channel. Requires explicit user \
                confirmation before sending. Use teams_channels to find channel IDs first. \
                Requires ChannelMessage.Send Azure AD permission."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "Teams team ID (GUID). Optional if [teams].team_id is configured."
                    },
                    "channel_id": {
                        "type": "string",
                        "description": "Teams channel ID (GUID). Required."
                    },
                    "message": {
                        "type": "string",
                        "description": "Message content to send (plain text)."
                    }
                },
                "required": ["channel_id", "message"]
            }),
        },
        ToolDefinition {
            name: "teams_presence".into(),
            description: "Check a Microsoft Teams user's presence and availability status. \
                Returns availability (Available, Busy, DoNotDisturb, Away, Offline) and \
                activity (InACall, InAMeeting, Presenting, etc.). \
                Requires Presence.Read.All Azure AD permission."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "user": {
                        "type": "string",
                        "description": "User email/UPN (e.g. sarah@civalent.com) or Azure AD object ID."
                    }
                },
                "required": ["user"]
            }),
        },
    ]
}

// ── Internal Helpers ─────────────────────────────────────────────────

/// Naive HTML tag stripper for Teams message bodies.
fn strip_html(html: &str) -> String {
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
    result.trim().to_string()
}

/// Truncate a string to at most `max_chars` Unicode scalar values, appending `…` if cut.
fn truncate_to_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{collected}…")
    } else {
        collected
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nv_core::config::Secrets;
    use std::collections::HashMap;

    fn make_secrets(tenant: Option<&str>, client_id: Option<&str>, secret: Option<&str>) -> Secrets {
        Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: None,
            bluebubbles_password: None,
            ms_graph_client_id: client_id.map(String::from),
            ms_graph_client_secret: secret.map(String::from),
            ms_graph_tenant_id: tenant.map(String::from),
            jira_api_token: None,
            jira_username: None,
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
            google_calendar_credentials: None,
        }
    }

    #[test]
    fn build_teams_client_succeeds_with_valid_secrets() {
        let secrets = make_secrets(
            Some("tenant-123"),
            Some("client-456"),
            Some("secret-789"),
        );
        let result = build_teams_client(&secrets, None);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
    }

    #[test]
    fn build_teams_client_uses_config_tenant_when_env_missing() {
        let secrets = make_secrets(None, Some("client-1"), Some("secret-1"));
        let config = TeamsConfig {
            tenant_id: "tenant-from-config".to_string(),
            team_id: None,
            team_ids: vec![],
            channel_ids: vec![],
            webhook_url: None,
        };
        let result = build_teams_client(&secrets, Some(&config));
        assert!(result.is_ok());
    }

    #[test]
    fn build_teams_client_fails_when_tenant_missing() {
        let secrets = make_secrets(None, Some("client-1"), Some("secret-1"));
        let result = build_teams_client(&secrets, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("tenant ID"), "Error should mention tenant ID: {err}");
    }

    #[test]
    fn build_teams_client_fails_when_client_id_missing() {
        let secrets = make_secrets(Some("tenant-1"), None, Some("secret-1"));
        let result = build_teams_client(&secrets, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("MS_GRAPH_CLIENT_ID"), "Error should mention CLIENT_ID: {err}");
    }

    #[test]
    fn build_teams_client_fails_when_secret_missing() {
        let secrets = make_secrets(Some("tenant-1"), Some("client-1"), None);
        let result = build_teams_client(&secrets, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("MS_GRAPH_CLIENT_SECRET"), "Error should mention CLIENT_SECRET: {err}");
    }

    #[test]
    fn teams_tool_definitions_returns_four_tools() {
        let tools = teams_tool_definitions();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn teams_tool_definitions_correct_names() {
        let tools = teams_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"teams_channels"));
        assert!(names.contains(&"teams_messages"));
        assert!(names.contains(&"teams_send"));
        assert!(names.contains(&"teams_presence"));
    }

    #[test]
    fn teams_tool_definitions_have_valid_schemas() {
        let tools = teams_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn teams_messages_requires_channel_id() {
        let tools = teams_tool_definitions();
        let msg_tool = tools.iter().find(|t| t.name == "teams_messages").unwrap();
        let required = msg_tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("channel_id")));
        // team_id is optional
        assert!(!required.iter().any(|v| v.as_str() == Some("team_id")));
    }

    #[test]
    fn teams_send_requires_channel_id_and_message() {
        let tools = teams_tool_definitions();
        let send_tool = tools.iter().find(|t| t.name == "teams_send").unwrap();
        let required = send_tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("channel_id")));
        assert!(required.iter().any(|v| v.as_str() == Some("message")));
    }

    #[test]
    fn teams_presence_requires_user() {
        let tools = teams_tool_definitions();
        let presence_tool = tools.iter().find(|t| t.name == "teams_presence").unwrap();
        let required = presence_tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("user")));
    }

    #[test]
    fn truncate_to_chars_no_truncation_when_short() {
        assert_eq!(truncate_to_chars("hello", 200), "hello");
        assert_eq!(truncate_to_chars("", 200), "");
    }

    #[test]
    fn truncate_to_chars_truncates_long_string() {
        let s = "a".repeat(250);
        let result = truncate_to_chars(&s, 200);
        // 200 'a' chars + '…' = 201 chars in result
        let char_count = result.chars().count();
        assert_eq!(char_count, 201);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_to_chars_exact_boundary() {
        let s = "a".repeat(200);
        let result = truncate_to_chars(&s, 200);
        assert_eq!(result, s);
        assert!(!result.ends_with('…'));
    }

    #[test]
    fn strip_html_removes_tags() {
        assert_eq!(strip_html("<p>Hello <b>world</b>!</p>"), "Hello world!");
        assert_eq!(strip_html("no tags here"), "no tags here");
        assert_eq!(strip_html("<div><span>nested</span></div>"), "nested");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn presence_formatting() {
        // Simulate what teams_presence returns
        let user = "sarah@civalent.com";
        let availability = "Available";
        let activity = "InACall";
        let output = format!("{user}: {availability} — {activity}");
        assert_eq!(output, "sarah@civalent.com: Available — InACall");
    }
}

// ── TeamsCheck wrapper ───────────────────────────────────────────────

/// Zero-arg probe struct for `nv check`.
///
/// Validates all three MS Graph credentials by acquiring an OAuth token
/// via the client credentials flow.
#[allow(dead_code)]
pub struct TeamsCheck;

// ── Checkable ────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl crate::tools::Checkable for TeamsCheck {
    fn name(&self) -> &str {
        "teams"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;

        // Check all three required env vars; return Missing on the first absent one.
        for var in &[
            "MS_GRAPH_CLIENT_ID",
            "MS_GRAPH_CLIENT_SECRET",
            "MS_GRAPH_TENANT_ID",
        ] {
            if std::env::var(var).is_err() {
                return crate::tools::CheckResult::Missing {
                    env_var: (*var).into(),
                };
            }
        }

        // All vars present — safe to unwrap.
        let client_id = std::env::var("MS_GRAPH_CLIENT_ID").unwrap();
        let client_secret = std::env::var("MS_GRAPH_CLIENT_SECRET").unwrap();
        let tenant_id = std::env::var("MS_GRAPH_TENANT_ID").unwrap();

        let auth = MsGraphAuth::new(&tenant_id, &client_id, &client_secret);

        let (latency, result) = timed(std::time::Duration::from_secs(15), || async { auth.authenticate().await }).await;

        match result {
            Ok(()) => crate::tools::CheckResult::Healthy {
                latency_ms: latency,
                detail: "OAuth token acquired".into(),
            },
            Err(e) => crate::tools::CheckResult::Unhealthy {
                error: e.to_string(),
            },
        }
    }
}
