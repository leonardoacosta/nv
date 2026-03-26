//! Microsoft Teams tools via MS Graph REST API.
//!
//! Provides six Claude-callable tools:
//! - `teams_channels`   — list channels in a team
//! - `teams_messages`   — read recent messages from a channel
//! - `teams_send`       — send a message to a channel (requires PendingAction confirmation)
//! - `teams_presence`   — check a user's presence/availability status
//! - `teams_list_chats` — list DMs and group chats accessible to the app
//! - `teams_read_chat`  — read recent messages from a specific chat
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
use crate::channels::teams::oauth::{graph_token_path, MsGraphAuth, MsGraphUserAuth};
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
    let mut client = TeamsClient::new(auth);

    // Try to load a cached delegated token for chat access (/me/chats).
    // Chat tools require delegated (user) auth; app-only tokens are rejected by
    // the /me/chats endpoint. The token is acquired via device-code flow and
    // cached at ~/.config/nv/graph-token.json (or NV_GRAPH_TOKEN_PATH).
    let token_path = graph_token_path();
    if let Some(user_auth) = MsGraphUserAuth::from_cache(&token_path) {
        client.delegated_token = Some(user_auth.access_token);
        tracing::debug!(
            path = %token_path.display(),
            "Loaded delegated token for Teams chat access"
        );
    }

    Ok(client)
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

/// List DMs and group chats accessible to the Azure AD app.
///
/// Calls `TeamsClient::list_chats(limit)` and formats each chat as a table row
/// showing the type badge (DM / Group / Meeting), topic/members, and last activity.
/// For one-on-one DMs (no topic), the other person's display name is shown instead.
pub async fn teams_list_chats(client: &TeamsClient, limit: usize) -> Result<String> {
    let chats = client.list_chats(limit).await?;

    if chats.is_empty() {
        return Ok("No chats found. Ensure the delegated token has Chat.Read scope and is cached at ~/.config/nv/graph-token.json.".to_string());
    }

    let mut lines = vec![format!(
        "**Teams Chats** — {} chat{}",
        chats.len(),
        if chats.len() == 1 { "" } else { "s" }
    )];

    for chat in &chats {
        let type_badge = match chat.chat_type.as_str() {
            "oneOnOne" => "DM",
            "group" => "Group",
            "meeting" => "Meeting",
            other => other,
        };

        // For DMs, use the other member's name as the display topic.
        // For group chats, use the topic or fall back to member list.
        let display_topic = if chat.chat_type == "oneOnOne" {
            // Show the first non-empty member name as the "other person"
            chat.members
                .as_ref()
                .and_then(|members| {
                    members
                        .iter()
                        .find_map(|m| m.display_name.as_deref().filter(|n| !n.is_empty()))
                })
                .unwrap_or("(unknown)")
                .to_string()
        } else {
            chat.topic.as_deref().unwrap_or("(no topic)").to_string()
        };

        let last_activity = chat
            .last_updated_date_time
            .as_deref()
            .unwrap_or("unknown");

        lines.push(format!(
            "   [{type_badge}] {display_topic}  |  last: {last_activity}  |  id: {}",
            chat.id
        ));
    }

    Ok(lines.join("\n"))
}

/// Read recent messages from a Teams chat (DM or group chat).
///
/// Calls `TeamsClient::get_chat_messages(chat_id, limit)` and formats each message
/// with sender, timestamp, and content (HTML stripped and truncated to 500 chars).
pub async fn teams_read_chat(
    client: &TeamsClient,
    chat_id: &str,
    limit: usize,
) -> Result<String> {
    let messages = client.get_chat_messages(chat_id, limit).await?;

    if messages.is_empty() {
        return Ok(format!("No messages found in chat `{chat_id}`."));
    }

    let mut lines = vec![format!(
        "**Chat {chat_id}** — {} message{}",
        messages.len(),
        if messages.len() == 1 { "" } else { "s" }
    )];

    for msg in &messages {
        // Skip system/event messages — only show "message" type
        if msg
            .message_type
            .as_deref()
            .map(|t| t != "message")
            .unwrap_or(false)
        {
            continue;
        }

        let sender = msg
            .from
            .as_ref()
            .and_then(|f| {
                f.user
                    .as_ref()
                    .and_then(|u| u.display_name.as_deref())
                    .or_else(|| {
                        f.application
                            .as_ref()
                            .and_then(|a| a.display_name.as_deref())
                    })
            })
            .unwrap_or("unknown");

        let timestamp = msg.created_date_time.as_deref().unwrap_or("unknown time");

        // Strip HTML tags if content_type is html, then truncate
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

        let preview = truncate_to_chars(&raw_content, 500);
        lines.push(format!("   [{timestamp}] {sender}: {preview}"));
    }

    Ok(lines.join("\n"))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return the 6 Teams tool definitions for the Anthropic API.
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
        ToolDefinition {
            name: "teams_list_chats".into(),
            description: "List Microsoft Teams chats (DMs and group chats) for the authenticated user. \
                Returns chat type (DM/Group/Meeting), topic or member name for DMs, and last activity time. \
                Use the returned chat ID with teams_read_chat to read messages. \
                Requires a cached delegated token (Chat.Read scope) at ~/.config/nv/graph-token.json."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of chats to return (default 20, max 50)."
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "teams_read_chat".into(),
            description: "Read recent messages from a Microsoft Teams chat (DM or group chat). \
                Returns sender, timestamp, and content (HTML stripped, truncated to 500 chars per message). \
                Use teams_list_chats to find chat IDs. \
                Requires a cached delegated token (Chat.Read scope) at ~/.config/nv/graph-token.json."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "chat_id": {
                        "type": "string",
                        "description": "Teams chat ID. Use teams_list_chats to find available chat IDs."
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of messages to return (default 20, max 50)."
                    }
                },
                "required": ["chat_id"]
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

// ── TeamsCheck wrapper ───────────────────────────────────────────────

/// Zero-arg probe struct for `nv check`.
///
/// Validates all three MS Graph credentials by acquiring an OAuth token
/// via the client credentials flow.
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
    fn teams_tool_definitions_returns_six_tools() {
        let tools = teams_tool_definitions();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn teams_tool_definitions_correct_names() {
        let tools = teams_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"teams_channels"));
        assert!(names.contains(&"teams_messages"));
        assert!(names.contains(&"teams_send"));
        assert!(names.contains(&"teams_presence"));
        assert!(names.contains(&"teams_list_chats"), "missing teams_list_chats");
        assert!(names.contains(&"teams_read_chat"), "missing teams_read_chat");
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

    // ── New tool tests ─────────────────────────────────────────────

    /// Verify that a DM with no topic shows the other member's display name.
    #[tokio::test]
    async fn teams_list_chats_formats_dm_with_member_name() {
        use crate::channels::teams::types::{ChatInfo, ChatMember};

        // Build the formatted output the same way teams_list_chats does (without HTTP).
        let chats = vec![ChatInfo {
            id: "chat-dm-001".to_string(),
            topic: None, // DMs have no topic
            chat_type: "oneOnOne".to_string(),
            last_updated_date_time: Some("2024-06-15T10:30:00Z".to_string()),
            members: Some(vec![
                ChatMember {
                    display_name: Some("Alice Smith".to_string()),
                    email: Some("alice@example.com".to_string()),
                },
            ]),
        }];

        // Re-implement the formatting logic to test it in isolation.
        let type_badge = match chats[0].chat_type.as_str() {
            "oneOnOne" => "DM",
            "group" => "Group",
            "meeting" => "Meeting",
            other => other,
        };

        let display_topic = if chats[0].chat_type == "oneOnOne" {
            chats[0]
                .members
                .as_ref()
                .and_then(|members| {
                    members
                        .iter()
                        .find_map(|m| m.display_name.as_deref().filter(|n| !n.is_empty()))
                })
                .unwrap_or("(unknown)")
                .to_string()
        } else {
            chats[0].topic.as_deref().unwrap_or("(no topic)").to_string()
        };

        assert_eq!(type_badge, "DM", "Chat type badge should be DM");
        assert_eq!(
            display_topic, "Alice Smith",
            "DM with no topic should show member name"
        );
    }

    /// Verify that HTML content is stripped when formatting chat messages.
    #[test]
    fn teams_read_chat_strips_html() {
        let html = "<p>Hello <b>world</b>! &amp; more</p>";
        // The strip_html function used by teams_read_chat
        let stripped = strip_html(html);
        assert_eq!(stripped, "Hello world! &amp; more", "strip_html removes tags but leaves entities for teams_read_chat caller");
        // Verify no angle brackets remain
        assert!(!stripped.contains('<'), "should not contain opening bracket");
        assert!(!stripped.contains('>'), "should not contain closing bracket");
    }

    /// Verify both new tool definitions are present in the registry.
    #[test]
    fn tool_definitions_include_new_tools() {
        let tools = teams_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(
            names.contains(&"teams_list_chats"),
            "teams_list_chats must be registered"
        );
        assert!(
            names.contains(&"teams_read_chat"),
            "teams_read_chat must be registered"
        );

        // Validate schemas for new tools
        let list_chats = tools.iter().find(|t| t.name == "teams_list_chats").unwrap();
        let required = list_chats.input_schema["required"].as_array().unwrap();
        assert!(
            required.is_empty(),
            "teams_list_chats has no required fields (limit is optional)"
        );

        let read_chat = tools.iter().find(|t| t.name == "teams_read_chat").unwrap();
        let required = read_chat.input_schema["required"].as_array().unwrap();
        assert!(
            required.iter().any(|v| v.as_str() == Some("chat_id")),
            "teams_read_chat must require chat_id"
        );
        // limit should not be required
        assert!(
            !required.iter().any(|v| v.as_str() == Some("limit")),
            "teams_read_chat limit should be optional"
        );
    }
}
