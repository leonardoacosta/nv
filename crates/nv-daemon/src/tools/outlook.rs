//! Outlook email and calendar tools via SSH to CloudPC running PowerShell scripts.
//!
//! Two read-only tools:
//! * `read_outlook_inbox` — list recent messages from the Inbox.
//! * `read_outlook_calendar` — list upcoming calendar events.
//!
//! Both tools SSH into the CloudPC and run `graph-outlook.ps1`, which manages its
//! own device-code + token refresh flow. No token management is done here.

use anyhow::Result;

use crate::claude::ToolDefinition;
use crate::tools::cloudpc;

// ── Script name ────────────────────────────────────────────────────────

const OUTLOOK_SCRIPT: &str = "graph-outlook.ps1";

// ── Tool Handlers ─────────────────────────────────────────────────────

/// Fetch and format recent inbox messages via CloudPC.
///
/// `_limit` and `_unread_only` are accepted for API compatibility but the
/// script controls output format and count internally.
pub async fn read_inbox(
    _folder: Option<&str>,
    _count: u32,
    _unread_only: bool,
) -> Result<String> {
    cloudpc::ssh_cloudpc_script(OUTLOOK_SCRIPT, "").await
}

/// Fetch and format upcoming calendar events via CloudPC.
pub async fn read_calendar(_days_ahead: u32, _max_events: u32) -> Result<String> {
    cloudpc::ssh_cloudpc_script(OUTLOOK_SCRIPT, "").await
}

// ── Tool Definitions ─────────────────────────────────────────────────

pub fn outlook_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_outlook_inbox".into(),
            description: "Read recent emails from Outlook inbox. \
                Returns a formatted list of messages with sender, subject, timestamp, and preview. \
                Uses the CloudPC account's Outlook via PowerShell."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Mail folder to read (currently shows Inbox by default)."
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of messages to return (hint, default: 10).",
                        "minimum": 1,
                        "maximum": 25
                    },
                    "unread_only": {
                        "type": "boolean",
                        "description": "If true, prefer unread messages (default: false)."
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "read_outlook_calendar".into(),
            description: "Read upcoming events from Outlook calendar. \
                Returns calendar events with time, subject, and organizer. \
                Uses the CloudPC account's Outlook via PowerShell."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "days_ahead": {
                        "type": "integer",
                        "description": "How many days ahead to fetch events (default: 1).",
                        "minimum": 1,
                        "maximum": 30
                    },
                    "max_events": {
                        "type": "integer",
                        "description": "Maximum events to return (default: 10).",
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
    fn outlook_tool_definitions_count() {
        let defs = outlook_tool_definitions();
        assert_eq!(defs.len(), 2);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"read_outlook_inbox"));
        assert!(names.contains(&"read_outlook_calendar"));
    }

    #[test]
    fn outlook_tool_schemas_valid() {
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
    fn outlook_tools_have_no_required_fields() {
        let defs = outlook_tool_definitions();
        for tool in &defs {
            let required = tool.input_schema["required"].as_array().unwrap();
            assert!(
                required.is_empty(),
                "{} should have no required fields",
                tool.name
            );
        }
    }
}
