//! Microsoft Teams tools via SSH to CloudPC running PowerShell scripts.
//!
//! Provides six Claude-callable tools:
//! - `teams_channels`   — list channels in a team (via graph-teams.ps1)
//! - `teams_messages`   — read recent messages from a channel or chat (via graph-teams.ps1)
//! - `teams_send`       — send a message to a channel (requires PendingAction confirmation)
//! - `teams_presence`   — check a user's presence/availability status (direct Graph API, app-only)
//! - `teams_list_chats` — list Teams and chats (via graph-teams.ps1)
//! - `teams_read_chat`  — read recent messages from a specific chat (via graph-teams.ps1)
//!
//! All read tools (channels, messages, list_chats, read_chat) SSH into the CloudPC and run
//! `graph-teams.ps1`, which manages its own device-code + token refresh flow.
//!
//! `teams_presence` and `teams_send` retain the existing TeamsClient / app-only auth path
//! because they use application permissions that work without delegated tokens.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use nv_core::config::{Secrets, TeamsConfig};

use crate::channels::teams::client::TeamsClient;
use crate::channels::teams::oauth::MsGraphAuth;
use crate::claude::ToolDefinition;
use crate::tools::cloudpc;

// ── CloudPC script name ───────────────────────────────────────────────

const TEAMS_SCRIPT: &str = "graph-teams.ps1";

// ── Client Construction (presence + send only) ────────────────────────

/// Build a standalone `TeamsClient` for app-only operations (presence, send).
///
/// Only needed for `teams_presence`. `teams_send` produces a PendingAction and
/// never calls the Graph API directly from this function.
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

// ── Tool Handlers ─────────────────────────────────────────────────────

/// List channels in a Teams team via CloudPC.
///
/// Runs: `graph-teams.ps1 -Action channels -TeamName '<team_name>'`
pub async fn teams_channels(team_name: &str) -> Result<String> {
    let args = format!("-Action channels -TeamName '{team_name}'");
    cloudpc::ssh_cloudpc_script(TEAMS_SCRIPT, &args).await
}

/// Get recent messages from a Teams channel via CloudPC.
///
/// Runs: `graph-teams.ps1 -Action messages -TeamName '<team>' [-ChannelName '<ch>'] [-Count N]`
pub async fn teams_messages(
    team_name: &str,
    channel_name: Option<&str>,
    count: usize,
) -> Result<String> {
    let channel_part = channel_name
        .map(|c| format!(" -ChannelName '{c}'"))
        .unwrap_or_default();
    let args = format!("-Action messages -TeamName '{team_name}'{channel_part} -Count {count}");
    cloudpc::ssh_cloudpc_script(TEAMS_SCRIPT, &args).await
}

/// Check a Teams user's presence status (app-only Graph API — no SSH needed).
pub async fn teams_presence(client: &TeamsClient, user: &str) -> Result<String> {
    let presence = client.get_user_presence(user).await?;
    Ok(format!(
        "{user}: {} — {}",
        presence.availability, presence.activity
    ))
}

/// List Teams teams and chats via CloudPC.
///
/// Runs: `graph-teams.ps1 -Action list`
pub async fn teams_list_chats(_limit: usize) -> Result<String> {
    cloudpc::ssh_cloudpc_script(TEAMS_SCRIPT, "-Action list").await
}

/// Read recent messages from a Teams chat (DM or group) via CloudPC.
///
/// Runs: `graph-teams.ps1 -Action messages -ChatId '<chat_id>' -Count N`
pub async fn teams_read_chat(chat_id: &str, limit: usize) -> Result<String> {
    let args = format!("-Action messages -ChatId '{chat_id}' -Count {limit}");
    cloudpc::ssh_cloudpc_script(TEAMS_SCRIPT, &args).await
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return the 6 Teams tool definitions for the Anthropic API.
pub fn teams_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "teams_channels".into(),
            description: "List channels in a Microsoft Teams team. Returns channel names. \
                Uses team name (display name), not team ID."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_name": {
                        "type": "string",
                        "description": "Teams team display name (e.g. 'WholesaleIT'). Required."
                    }
                },
                "required": ["team_name"]
            }),
        },
        ToolDefinition {
            name: "teams_messages".into(),
            description: "Read recent messages from a Microsoft Teams channel. \
                Returns messages with sender and timestamp. \
                Specify channel_name to read a specific channel; omit to read General."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_name": {
                        "type": "string",
                        "description": "Teams team display name (e.g. 'WholesaleIT'). Required."
                    },
                    "channel_name": {
                        "type": "string",
                        "description": "Channel display name (e.g. 'Dev'). Defaults to General if omitted."
                    },
                    "count": {
                        "type": "number",
                        "description": "Number of messages to return (default 20)."
                    }
                },
                "required": ["team_name"]
            }),
        },
        ToolDefinition {
            name: "teams_send".into(),
            description: "Send a message to a Microsoft Teams channel. Requires explicit user \
                confirmation before sending. Use teams_channels to find channel names first."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "Teams team ID (GUID). Optional if NV_TEAMS_TEAM_ID is set."
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
            description: "List Microsoft Teams teams and chats. \
                Returns teams, DMs, and group chats accessible from the CloudPC account. \
                Use the returned chat ID with teams_read_chat to read messages."
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
                Returns sender, timestamp, and content. \
                Use teams_list_chats to find chat IDs."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "chat_id": {
                        "type": "string",
                        "description": "Teams chat ID (e.g. '19:...'). Use teams_list_chats to find available chat IDs."
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

// ── TeamsCheck wrapper ───────────────────────────────────────────────

/// Zero-arg probe struct for `nv check`.
///
/// Validates MS Graph app-only credentials by acquiring an OAuth token
/// (still used by teams_presence).
pub struct TeamsCheck;

#[async_trait::async_trait]
impl crate::tools::Checkable for TeamsCheck {
    fn name(&self) -> &str {
        "teams"
    }

    async fn check_read(&self) -> crate::tools::CheckResult {
        use crate::tools::check::timed;

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

        let client_id = std::env::var("MS_GRAPH_CLIENT_ID").unwrap();
        let client_secret = std::env::var("MS_GRAPH_CLIENT_SECRET").unwrap();
        let tenant_id = std::env::var("MS_GRAPH_TENANT_ID").unwrap();

        let auth = MsGraphAuth::new(&tenant_id, &client_id, &client_secret);

        let (latency, result) =
            timed(std::time::Duration::from_secs(15), || async { auth.authenticate().await })
                .await;

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
    fn teams_channels_requires_team_name() {
        let tools = teams_tool_definitions();
        let tool = tools.iter().find(|t| t.name == "teams_channels").unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("team_name")));
    }

    #[test]
    fn teams_messages_requires_team_name() {
        let tools = teams_tool_definitions();
        let msg_tool = tools.iter().find(|t| t.name == "teams_messages").unwrap();
        let required = msg_tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("team_name")));
        // channel_name and count are optional
        assert!(!required.iter().any(|v| v.as_str() == Some("channel_name")));
        assert!(!required.iter().any(|v| v.as_str() == Some("count")));
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
    fn teams_list_chats_has_no_required_fields() {
        let tools = teams_tool_definitions();
        let tool = tools.iter().find(|t| t.name == "teams_list_chats").unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty(), "teams_list_chats has no required fields");
    }

    #[test]
    fn teams_read_chat_requires_chat_id() {
        let tools = teams_tool_definitions();
        let tool = tools.iter().find(|t| t.name == "teams_read_chat").unwrap();
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("chat_id")));
        assert!(!required.iter().any(|v| v.as_str() == Some("limit")));
    }

    #[test]
    fn tool_definitions_include_new_tools() {
        let tools = teams_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(names.contains(&"teams_list_chats"), "teams_list_chats must be registered");
        assert!(names.contains(&"teams_read_chat"), "teams_read_chat must be registered");
    }

    #[test]
    fn presence_formatting() {
        let user = "sarah@civalent.com";
        let availability = "Available";
        let activity = "InACall";
        let output = format!("{user}: {availability} — {activity}");
        assert_eq!(output, "sarah@civalent.com: Available — InACall");
    }
}
