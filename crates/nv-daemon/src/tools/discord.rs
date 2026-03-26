//! Discord read tools via the Discord REST API.
//!
//! Provides three Claude-callable tools:
//! - `discord_list_guilds`    — list Discord servers the bot is in
//! - `discord_list_channels`  — list text channels in a server, grouped by category
//! - `discord_read_messages`  — read recent messages from a channel
//!
//! Auth: reads `DISCORD_BOT_TOKEN` from `secrets.discord_bot_token`.
//! The bot token is the same credential used by the gateway adapter.
//!
//! This module is separate from the channel adapter (inbound gateway relay).
//! Tools construct a standalone `DiscordRestClient` for read-only API calls.

use anyhow::{anyhow, Result};
use nv_core::config::Secrets;

use crate::channels::discord::client::DiscordRestClient;
use crate::claude::ToolDefinition;

// ── Client Construction ───────────────────────────────────────────────

/// Build a standalone `DiscordRestClient` for tool use.
///
/// Reads `DISCORD_BOT_TOKEN` from `secrets.discord_bot_token`.
/// Returns an error if the token is not configured.
pub fn build_discord_client(secrets: &Secrets) -> Result<DiscordRestClient> {
    let token = secrets
        .discord_bot_token
        .as_deref()
        .ok_or_else(|| anyhow!("DISCORD_BOT_TOKEN not set"))?;
    Ok(DiscordRestClient::new(token))
}

// ── Tool Handlers ────────────────────────────────────────────────────

/// List all Discord servers (guilds) the bot is a member of.
///
/// Calls `DiscordRestClient::list_guilds()` and formats the result as a
/// readable list with server name and ID.
pub async fn discord_list_guilds(client: &DiscordRestClient) -> Result<String> {
    let guilds = client.list_guilds().await.map_err(|e| {
        if e.to_string().contains("401") {
            anyhow!("Discord auth invalid (401). DISCORD_BOT_TOKEN may be wrong or expired.")
        } else if e.to_string().contains("403") {
            anyhow!("Insufficient permissions to list guilds (403).")
        } else {
            e
        }
    })?;

    if guilds.is_empty() {
        return Ok("Bot is not a member of any Discord servers.".to_string());
    }

    let mut lines = vec![format!(
        "Discord Servers — {} server{}",
        guilds.len(),
        if guilds.len() == 1 { "" } else { "s" }
    )];
    for guild in &guilds {
        lines.push(format!("   • {} (id: {})", guild.name, guild.id));
    }

    Ok(lines.join("\n"))
}

/// List text channels in a Discord guild, grouped by category.
///
/// Calls `DiscordRestClient::list_channels(guild_id)` and formats channels
/// under their parent category name. Channels without a category are listed
/// under "Uncategorized".
pub async fn discord_list_channels(
    client: &DiscordRestClient,
    guild_id: &str,
) -> Result<String> {
    let channels = client.list_channels(guild_id).await.map_err(|e| {
        if e.to_string().contains("403") {
            anyhow!("Access denied to guild '{}' channels (403). Bot may lack permissions.", guild_id)
        } else if e.to_string().contains("404") {
            anyhow!("Guild '{}' not found (404).", guild_id)
        } else {
            e
        }
    })?;

    if channels.is_empty() {
        return Ok(format!("No text channels found in guild `{guild_id}`."));
    }

    // Group channels by parent_id (category).
    // We need category names — fetch raw to get category channels (type 4).
    // Since list_channels already filtered to type 0, we build a simple grouping
    // using parent_id as the key; show parent_id as the group label.
    let mut groups: std::collections::BTreeMap<String, Vec<&crate::channels::discord::types::DiscordChannel>> =
        std::collections::BTreeMap::new();

    for ch in &channels {
        let category_key = ch
            .parent_id
            .as_deref()
            .unwrap_or("uncategorized")
            .to_string();
        groups.entry(category_key).or_default().push(ch);
    }

    let mut lines = vec![format!(
        "Channels in guild `{guild_id}` — {} text channel{}",
        channels.len(),
        if channels.len() == 1 { "" } else { "s" }
    )];

    for (category, chs) in &groups {
        let label = if category == "uncategorized" {
            "Uncategorized".to_string()
        } else {
            format!("Category {category}")
        };
        lines.push(format!("  [{label}]"));
        for ch in chs {
            let topic = ch
                .topic
                .as_deref()
                .map(|t| format!(" — {t}"))
                .unwrap_or_default();
            lines.push(format!("    • #{} ({}){}", ch.name, ch.id, topic));
        }
    }

    Ok(lines.join("\n"))
}

/// Read recent messages from a Discord channel.
///
/// Calls `DiscordRestClient::get_messages(channel_id, limit)` and formats
/// each message as `[timestamp] author: content` with content truncated to
/// 500 characters. Empty content (embed-only messages) shows `[embed]`.
pub async fn discord_read_messages(
    client: &DiscordRestClient,
    channel_id: &str,
    limit: usize,
) -> Result<String> {
    let messages = client.get_messages(channel_id, limit).await.map_err(|e| {
        if e.to_string().contains("403") {
            anyhow!(
                "Access denied to channel '{}' (403). Bot may lack read permissions.",
                channel_id
            )
        } else if e.to_string().contains("404") {
            anyhow!("Channel '{}' not found (404).", channel_id)
        } else {
            e
        }
    })?;

    if messages.is_empty() {
        return Ok(format!("No messages found in channel `{channel_id}`."));
    }

    let mut lines = vec![format!(
        "Messages in channel `{channel_id}` — {} message{}",
        messages.len(),
        if messages.len() == 1 { "" } else { "s" }
    )];

    for msg in &messages {
        let author = msg
            .author
            .global_name
            .as_deref()
            .unwrap_or(&msg.author.username);

        let content = if msg.content.is_empty() {
            let attach_count = msg
                .attachments
                .as_ref()
                .map(|a| a.len())
                .unwrap_or(0);
            if attach_count > 0 {
                format!("[{attach_count} attachment(s)]")
            } else {
                "[embed]".to_string()
            }
        } else {
            truncate_to_chars(&msg.content, 500)
        };

        lines.push(format!("   [{}] {}: {}", msg.timestamp, author, content));
    }

    Ok(lines.join("\n"))
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return the 3 Discord read tool definitions for the Anthropic API.
pub fn discord_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "discord_list_guilds".into(),
            description: "List all Discord servers (guilds) the bot is a member of. \
                Returns server names and IDs. Use this to discover available servers \
                before listing channels or reading messages."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "discord_list_channels".into(),
            description: "List text channels in a Discord server, grouped by category. \
                Returns channel names, IDs, and topics. Use discord_list_guilds first \
                to find guild IDs. Only returns text channels (type 0)."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "guild_id": {
                        "type": "string",
                        "description": "Discord guild (server) ID. Use discord_list_guilds to find available IDs."
                    }
                },
                "required": ["guild_id"]
            }),
        },
        ToolDefinition {
            name: "discord_read_messages".into(),
            description: "Read recent messages from a Discord channel. Returns messages \
                newest-first with author, timestamp, and content (truncated to 500 chars). \
                Use discord_list_channels to find channel IDs."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel_id": {
                        "type": "string",
                        "description": "Discord channel ID. Use discord_list_channels to find available channel IDs."
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of messages to return (default 20, max 50)."
                    }
                },
                "required": ["channel_id"]
            }),
        },
    ]
}

// ── Internal Helpers ─────────────────────────────────────────────────

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
    use crate::channels::discord::types::{DiscordAuthor, DiscordChannel, DiscordMessage};
    use std::collections::HashMap;

    fn make_secrets(token: Option<&str>) -> Secrets {
        Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: token.map(String::from),
            bluebubbles_password: None,
            ms_graph_client_id: None,
            ms_graph_client_secret: None,
            ms_graph_tenant_id: None,
            jira_api_token: None,
            jira_username: None,
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
            google_calendar_credentials: None,
        }
    }

    #[test]
    fn build_discord_client_succeeds_with_token() {
        let secrets = make_secrets(Some("test-token"));
        let result = build_discord_client(&secrets);
        assert!(result.is_ok(), "Expected Ok but got Err");
    }

    #[test]
    fn build_discord_client_fails_without_token() {
        let secrets = make_secrets(None);
        let result = build_discord_client(&secrets);
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("DISCORD_BOT_TOKEN"),
            "Error should mention DISCORD_BOT_TOKEN: {err}"
        );
    }

    #[test]
    fn discord_tool_definitions_returns_three_tools() {
        let tools = discord_tool_definitions();
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn discord_tool_definitions_correct_names() {
        let tools = discord_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"discord_list_guilds"), "missing discord_list_guilds");
        assert!(names.contains(&"discord_list_channels"), "missing discord_list_channels");
        assert!(names.contains(&"discord_read_messages"), "missing discord_read_messages");
    }

    #[test]
    fn discord_tool_definitions_have_valid_schemas() {
        let tools = discord_tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn discord_list_channels_requires_guild_id() {
        let tools = discord_tool_definitions();
        let ch_tool = tools.iter().find(|t| t.name == "discord_list_channels").unwrap();
        let required = ch_tool.input_schema["required"].as_array().unwrap();
        assert!(
            required.iter().any(|v| v.as_str() == Some("guild_id")),
            "discord_list_channels must require guild_id"
        );
    }

    #[test]
    fn discord_read_messages_requires_channel_id() {
        let tools = discord_tool_definitions();
        let msg_tool = tools.iter().find(|t| t.name == "discord_read_messages").unwrap();
        let required = msg_tool.input_schema["required"].as_array().unwrap();
        assert!(
            required.iter().any(|v| v.as_str() == Some("channel_id")),
            "discord_read_messages must require channel_id"
        );
        assert!(
            !required.iter().any(|v| v.as_str() == Some("limit")),
            "discord_read_messages limit should be optional"
        );
    }

    #[test]
    fn discord_list_guilds_requires_no_params() {
        let tools = discord_tool_definitions();
        let guilds_tool = tools.iter().find(|t| t.name == "discord_list_guilds").unwrap();
        let required = guilds_tool.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty(), "discord_list_guilds has no required fields");
    }

    // ── Formatting tests (no HTTP) ────────────────────────────────────

    #[test]
    fn discord_list_channels_formats_grouped_by_category() {
        // Simulate what discord_list_channels formats — test the grouping logic directly.
        let channels = vec![
            DiscordChannel {
                id: "ch-1".to_string(),
                name: "general".to_string(),
                channel_type: 0,
                topic: Some("Main chat".to_string()),
                position: Some(0),
                parent_id: Some("cat-1".to_string()),
            },
            DiscordChannel {
                id: "ch-2".to_string(),
                name: "announcements".to_string(),
                channel_type: 0,
                topic: None,
                position: Some(1),
                parent_id: Some("cat-1".to_string()),
            },
            DiscordChannel {
                id: "ch-3".to_string(),
                name: "bot-commands".to_string(),
                channel_type: 0,
                topic: None,
                position: Some(0),
                parent_id: None,
            },
        ];

        // Build grouping output (same logic as discord_list_channels)
        let mut groups: std::collections::BTreeMap<String, Vec<&DiscordChannel>> =
            std::collections::BTreeMap::new();
        for ch in &channels {
            let key = ch.parent_id.as_deref().unwrap_or("uncategorized").to_string();
            groups.entry(key).or_default().push(ch);
        }

        assert!(groups.contains_key("cat-1"), "should have cat-1 group");
        assert!(groups.contains_key("uncategorized"), "should have uncategorized group");
        assert_eq!(groups["cat-1"].len(), 2);
        assert_eq!(groups["uncategorized"].len(), 1);
    }

    #[test]
    fn discord_read_messages_truncates_long_content() {
        let long_content = "a".repeat(600);
        let truncated = truncate_to_chars(&long_content, 500);
        let char_count = truncated.chars().count();
        // 500 'a' chars + '…' = 501 chars
        assert_eq!(char_count, 501);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn discord_read_messages_no_truncation_when_short() {
        let short = "Hello from Discord!";
        let result = truncate_to_chars(short, 500);
        assert_eq!(result, short);
        assert!(!result.ends_with('…'));
    }

    #[test]
    fn discord_read_messages_shows_embed_for_empty_content() {
        let msg = DiscordMessage {
            id: "msg-1".to_string(),
            content: "".to_string(),
            author: DiscordAuthor {
                id: "u-1".to_string(),
                username: "testuser".to_string(),
                global_name: None,
            },
            timestamp: "2024-01-01T00:00:00+00:00".to_string(),
            attachments: None,
        };

        let content = if msg.content.is_empty() {
            let attach_count = msg.attachments.as_ref().map(|a| a.len()).unwrap_or(0);
            if attach_count > 0 {
                format!("[{attach_count} attachment(s)]")
            } else {
                "[embed]".to_string()
            }
        } else {
            truncate_to_chars(&msg.content, 500)
        };

        assert_eq!(content, "[embed]");
    }

    #[test]
    fn discord_read_messages_uses_global_name_when_present() {
        let msg = DiscordMessage {
            id: "msg-2".to_string(),
            content: "Hello!".to_string(),
            author: DiscordAuthor {
                id: "u-2".to_string(),
                username: "user_handle".to_string(),
                global_name: Some("Display Name".to_string()),
            },
            timestamp: "2024-01-01T00:00:00+00:00".to_string(),
            attachments: None,
        };

        let author = msg
            .author
            .global_name
            .as_deref()
            .unwrap_or(&msg.author.username);

        assert_eq!(author, "Display Name");
    }
}
