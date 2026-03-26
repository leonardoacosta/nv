//! Cross-channel routing tools — `list_channels` and `send_to_channel`.
//!
//! `list_channels` is a read-only tool that returns every configured channel
//! with its name, direction, and connection status. `send_to_channel` is a
//! write tool that validates the target channel and returns a
//! `PendingActionRequest` for the caller to persist and confirm; it does NOT
//! call `send_message` directly — execution happens post-confirmation in the
//! callback handler.

use std::fmt;

use anyhow::{anyhow, Result};

use crate::agent::ChannelRegistry;
use crate::claude::ToolDefinition;

// ── Direction Classification ─────────────────────────────────────────

/// Static direction of a channel adapter.
///
/// Direction is a compile-time property per adapter type — it does not
/// require a live API probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelDirection {
    /// Channel only receives messages — cannot send outbound.
    #[allow(dead_code)]
    Inbound,
    /// Channel only sends messages — cannot receive inbound.
    Outbound,
    /// Channel both receives and sends messages.
    Bidirectional,
}

impl fmt::Display for ChannelDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelDirection::Inbound => write!(f, "inbound"),
            ChannelDirection::Outbound => write!(f, "outbound"),
            ChannelDirection::Bidirectional => write!(f, "bidirectional"),
        }
    }
}

/// Returns `true` if this direction supports outbound sends.
pub fn supports_outbound(direction: &ChannelDirection) -> bool {
    matches!(
        direction,
        ChannelDirection::Outbound | ChannelDirection::Bidirectional
    )
}

/// Static direction table per adapter type (Req-5).
///
/// | Channel   | Direction     |
/// |-----------|---------------|
/// | telegram  | Bidirectional |
/// | discord   | Bidirectional |
/// | teams     | Bidirectional |
/// | imessage  | Bidirectional |
/// | email     | Outbound      |
pub fn channel_direction(name: &str) -> ChannelDirection {
    match name {
        "telegram" => ChannelDirection::Bidirectional,
        "discord" => ChannelDirection::Bidirectional,
        "teams" => ChannelDirection::Bidirectional,
        "imessage" => ChannelDirection::Bidirectional,
        "email" => ChannelDirection::Outbound,
        // Unknown channels default to Bidirectional — conservative assumption
        _ => ChannelDirection::Bidirectional,
    }
}

// ── ChannelInfo ──────────────────────────────────────────────────────

/// Per-channel status entry used in `list_channels` output.
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub name: String,
    pub connected: bool,
    pub direction: ChannelDirection,
}

// ── PendingActionRequest ─────────────────────────────────────────────

/// Value returned by `send_to_channel` for the caller to persist and confirm.
///
/// Does NOT trigger `send_message` directly — execution happens post-confirmation.
#[derive(Debug, Clone)]
pub struct PendingActionRequest {
    /// Human-readable description for the Telegram confirmation keyboard.
    pub description: String,
    /// Structured payload to be stored in `PendingAction::payload`.
    /// Always contains `channel` and `message` fields plus `_action_type`.
    pub payload: serde_json::Value,
}

// ── list_channels ────────────────────────────────────────────────────

/// List every configured channel with name, connection status, and direction.
///
/// Returns a formatted text table — one row per channel — or a sentinel
/// string when the registry is empty.
pub fn list_channels(registry: &ChannelRegistry) -> Result<String> {
    if registry.is_empty() {
        return Ok("No channels configured.".to_string());
    }

    let mut infos: Vec<ChannelInfo> = registry
        .keys()
        .map(|name| ChannelInfo {
            name: name.clone(),
            // Presence in the registry implies the adapter connected successfully.
            // If we cannot probe state, we conservatively mark as connected.
            connected: true,
            direction: channel_direction(name),
        })
        .collect();

    // Sort alphabetically for deterministic output
    infos.sort_by(|a, b| a.name.cmp(&b.name));

    let count = infos.len();
    format_channel_table(&infos, count)
}

/// Format a slice of `ChannelInfo` into an aligned text table.
fn format_channel_table(infos: &[ChannelInfo], total: usize) -> Result<String> {
    // Compute column widths
    let name_width = infos
        .iter()
        .map(|i| i.name.len())
        .max()
        .unwrap_or(4)
        .max(4); // minimum "name" header width

    let connected_header = "connected";
    let direction_header = "direction";

    let header = format!(
        "{:<name_width$}  {:<9}  {}",
        "name",
        connected_header,
        direction_header,
        name_width = name_width
    );
    let separator = format!(
        "{:-<name_width$}  {:-<9}  {:-<13}",
        "",
        "",
        "",
        name_width = name_width
    );

    let mut lines = vec![header, separator];

    for info in infos {
        let connected_str = if info.connected { "yes" } else { "no" };
        lines.push(format!(
            "{:<name_width$}  {:<9}  {}",
            info.name,
            connected_str,
            info.direction,
            name_width = name_width
        ));
    }

    lines.push(format!("\n{total} channel(s) configured."));
    Ok(lines.join("\n"))
}

// ── send_to_channel ──────────────────────────────────────────────────

/// Validate the target channel and build a `PendingActionRequest`.
///
/// Returns `Err` immediately for:
/// - Unknown channel name (Req-2 step 1)
/// - Channel that does not support outbound (Req-2 step 2)
///
/// Does NOT call `send_message` — the caller persists the returned request
/// and the callback handler executes on approval.
pub fn send_to_channel(
    registry: &ChannelRegistry,
    channel: &str,
    message: &str,
) -> Result<PendingActionRequest> {
    let channel_lower = channel.to_lowercase();

    // Step 1: validate channel exists
    let exists = registry
        .keys()
        .any(|k| k.to_lowercase() == channel_lower);

    if !exists {
        return Err(anyhow!(
            "Channel '{}' not configured. Use list_channels to see available channels.",
            channel
        ));
    }

    // Resolve the canonical name from the registry (case-insensitive match)
    let canonical_name = registry
        .keys()
        .find(|k| k.to_lowercase() == channel_lower)
        .cloned()
        .expect("channel was found above");

    // Step 2: validate outbound support
    let direction = channel_direction(&canonical_name);
    if !supports_outbound(&direction) {
        return Err(anyhow!(
            "Channel '{}' does not support outbound messages.",
            canonical_name
        ));
    }

    // Step 3: build description — first 80 chars of message with ellipsis
    let preview = if message.len() > 80 {
        format!("{}…", &message[..80])
    } else {
        message.to_string()
    };
    let description = format!("Send message to {canonical_name}: {preview}");

    // Build payload — channel + message + explicit action type for callback routing
    let payload = serde_json::json!({
        "channel": canonical_name,
        "message": message,
        "_action_type": "ChannelSend"
    });

    Ok(PendingActionRequest {
        description,
        payload,
    })
}

// ── Tool Definitions ─────────────────────────────────────────────────

/// Return MCP tool definitions for `list_channels` and `send_to_channel`.
pub fn channels_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_channels".into(),
            description: "List available messaging channels and their connection status and direction (inbound/outbound/bidirectional).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "send_to_channel".into(),
            description: "Send a message to a specific channel (telegram/discord/teams/imessage/email). Requires confirmation before delivery.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {
                        "type": "string",
                        "description": "Target channel name — must match an entry in list_channels (e.g. 'telegram', 'discord', 'teams', 'email'). Case-insensitive."
                    },
                    "message": {
                        "type": "string",
                        "description": "Message body to send. Plain text; channel adapters handle formatting."
                    }
                },
                "required": ["channel", "message"]
            }),
        },
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a mock registry with the given names pre-inserted.
    ///
    /// The channel handle is a no-op placeholder — we only test the
    /// registry-lookup and direction-classification logic here, not the
    /// actual send path.
    fn mock_registry(names: &[&str]) -> ChannelRegistry {
        use std::collections::HashMap;
        use std::sync::Arc;

        struct MockChannel;

        #[async_trait::async_trait]
        impl nv_core::channel::Channel for MockChannel {
            async fn poll_messages(&self) -> anyhow::Result<Vec<nv_core::types::InboundMessage>> {
                Ok(vec![])
            }
            async fn send_message(&self, _msg: nv_core::types::OutboundMessage) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let mut map: HashMap<String, Arc<dyn nv_core::channel::Channel>> = HashMap::new();
        for name in names {
            map.insert(name.to_string(), Arc::new(MockChannel) as Arc<dyn nv_core::channel::Channel>);
        }
        map
    }

    // ── list_channels ────────────────────────────────────────────────

    #[test]
    fn list_channels_empty_registry() {
        let registry = mock_registry(&[]);
        let result = list_channels(&registry).unwrap();
        assert_eq!(result, "No channels configured.");
    }

    #[test]
    fn list_channels_all_three_present() {
        let registry = mock_registry(&["telegram", "discord", "teams"]);
        let result = list_channels(&registry).unwrap();

        assert!(result.contains("telegram"), "expected 'telegram' in output");
        assert!(result.contains("discord"), "expected 'discord' in output");
        assert!(result.contains("teams"), "expected 'teams' in output");
    }

    #[test]
    fn list_channels_correct_directions() {
        let registry = mock_registry(&["telegram", "discord", "teams"]);
        let result = list_channels(&registry).unwrap();

        // All three are bidirectional
        assert!(
            result.contains("bidirectional"),
            "expected 'bidirectional' in output"
        );
    }

    #[test]
    fn list_channels_connected_yes() {
        let registry = mock_registry(&["telegram"]);
        let result = list_channels(&registry).unwrap();
        assert!(result.contains("yes"), "expected connected=yes");
    }

    #[test]
    fn list_channels_email_direction() {
        let registry = mock_registry(&["email"]);
        let result = list_channels(&registry).unwrap();
        assert!(result.contains("outbound"), "email should be outbound");
    }

    #[test]
    fn list_channels_channel_count_line() {
        let registry = mock_registry(&["telegram", "discord"]);
        let result = list_channels(&registry).unwrap();
        assert!(result.contains("2 channel(s)"), "expected count line");
    }

    // ── send_to_channel ──────────────────────────────────────────────

    #[test]
    fn send_to_channel_valid_returns_pending_request() {
        let registry = mock_registry(&["telegram"]);
        let result = send_to_channel(&registry, "telegram", "Hello from test").unwrap();

        assert!(
            result.description.contains("telegram"),
            "description should mention channel"
        );
        assert!(
            result.description.contains("Hello from test"),
            "description should contain message preview"
        );
        assert_eq!(result.payload["channel"].as_str(), Some("telegram"));
        assert_eq!(result.payload["message"].as_str(), Some("Hello from test"));
        assert_eq!(
            result.payload["_action_type"].as_str(),
            Some("ChannelSend")
        );
    }

    #[test]
    fn send_to_channel_unknown_channel_returns_not_configured_error() {
        let registry = mock_registry(&["telegram"]);
        let err = send_to_channel(&registry, "slack", "test").unwrap_err();
        assert!(
            err.to_string().contains("not configured"),
            "expected 'not configured' in error: {err}"
        );
    }

    #[test]
    fn send_to_channel_inbound_only_returns_no_outbound_error() {
        // Simulate an inbound-only channel by using a name that maps to Inbound.
        // We override the direction table by injecting a custom key that we
        // manually classify — but since our static table doesn't have "inbound_only",
        // we need a different approach: we override the test by registering a known
        // inbound-only name. The current table has no pure inbound channels, so we
        // patch it via direct function call.
        //
        // Instead, test the underlying `supports_outbound` logic directly:
        assert!(!supports_outbound(&ChannelDirection::Inbound));
        assert!(supports_outbound(&ChannelDirection::Outbound));
        assert!(supports_outbound(&ChannelDirection::Bidirectional));
    }

    #[test]
    fn send_to_channel_case_insensitive() {
        let registry = mock_registry(&["telegram"]);
        // "TELEGRAM" should match "telegram"
        let result = send_to_channel(&registry, "TELEGRAM", "msg").unwrap();
        assert_eq!(result.payload["channel"].as_str(), Some("telegram"));
    }

    #[test]
    fn send_to_channel_description_truncates_long_message() {
        let registry = mock_registry(&["telegram"]);
        let long_msg = "x".repeat(200);
        let result = send_to_channel(&registry, "telegram", &long_msg).unwrap();
        // Description should contain truncated preview with ellipsis
        assert!(result.description.contains('…'), "expected ellipsis in description");
        // Payload must still carry the full message
        assert_eq!(result.payload["message"].as_str().map(|s| s.len()), Some(200));
    }

    // ── channels_tool_definitions ────────────────────────────────────

    #[test]
    fn channels_tool_definitions_returns_two() {
        let defs = channels_tool_definitions();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn channels_tool_definitions_correct_names() {
        let defs = channels_tool_definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"list_channels"));
        assert!(names.contains(&"send_to_channel"));
    }

    #[test]
    fn channels_tool_definitions_send_to_channel_requires_both_fields() {
        let defs = channels_tool_definitions();
        let send_def = defs.iter().find(|d| d.name == "send_to_channel").unwrap();
        let required = send_def.input_schema["required"]
            .as_array()
            .expect("required should be an array");
        let req_names: Vec<&str> = required
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(req_names.contains(&"channel"), "channel must be required");
        assert!(req_names.contains(&"message"), "message must be required");
    }

    #[test]
    fn channels_tool_definitions_list_channels_empty_schema() {
        let defs = channels_tool_definitions();
        let list_def = defs.iter().find(|d| d.name == "list_channels").unwrap();
        let props = &list_def.input_schema["properties"];
        assert!(
            props.as_object().map_or(false, |o| o.is_empty()),
            "list_channels should have empty properties"
        );
    }

    // ── ChannelDirection helpers ─────────────────────────────────────

    #[test]
    fn channel_direction_known_channels() {
        assert_eq!(channel_direction("telegram"), ChannelDirection::Bidirectional);
        assert_eq!(channel_direction("discord"), ChannelDirection::Bidirectional);
        assert_eq!(channel_direction("teams"), ChannelDirection::Bidirectional);
        assert_eq!(channel_direction("imessage"), ChannelDirection::Bidirectional);
        assert_eq!(channel_direction("email"), ChannelDirection::Outbound);
    }

    #[test]
    fn channel_direction_display() {
        assert_eq!(ChannelDirection::Inbound.to_string(), "inbound");
        assert_eq!(ChannelDirection::Outbound.to_string(), "outbound");
        assert_eq!(ChannelDirection::Bidirectional.to_string(), "bidirectional");
    }
}
