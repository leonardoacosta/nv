pub use nv_tools::tools::ado;
pub use nv_tools::tools::calendar;
pub mod channels;
pub mod check;
pub mod checkable_impls;
pub mod discord;
pub mod outlook;
pub use nv_tools::tools::cloudflare;
pub use nv_tools::tools::docker;
pub use nv_tools::tools::doppler;
pub use nv_tools::tools::github;
pub use nv_tools::tools::ha;
pub mod jira;
pub use nv_tools::tools::neon;
pub use nv_tools::tools::plaid;
pub use nv_tools::tools::posthog;
pub use nv_tools::tools::resend;
pub mod schedule;
pub use nv_tools::tools::sentry;
pub use nv_tools::tools::stripe;
pub mod teams;
pub use nv_tools::tools::upstash;
pub use nv_tools::tools::vercel;
pub use nv_tools::tools::web;

// ── Checkable trait ─────────────────────────────────────────────────

/// Result of a single service connectivity probe.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CheckResult {
    /// Service responded successfully within the timeout.
    Healthy {
        /// Round-trip time in milliseconds.
        latency_ms: u64,
        /// Human-readable detail (e.g. authenticated account, version).
        detail: String,
    },
    /// Service is reachable but something is wrong (e.g. auth degraded, quota).
    Degraded {
        /// Human-readable description of the degraded state.
        message: String,
    },
    /// Service is unreachable or returned a fatal error.
    Unhealthy {
        /// Error description.
        error: String,
    },
    /// Required credential (env var) is absent — service was never configured.
    Missing {
        /// The environment variable that is not set.
        env_var: String,
    },
}

/// A service that can validate its own connectivity and credentials.
#[async_trait::async_trait]
pub trait Checkable: Send + Sync {
    /// Human-readable service name, e.g. `"stripe"` or `"jira/personal"`.
    fn name(&self) -> &str;

    /// Check read connectivity — lightweight GET or equivalent.
    async fn check_read(&self) -> CheckResult;

    /// Check write permissions — dry-run probe (expect 4xx, not 2xx).
    ///
    /// Returns `None` if the service has no writable endpoints to probe.
    async fn check_write(&self) -> Option<CheckResult> {
        None
    }
}

// ── ServiceRegistry<T> ───────────────────────────────────────────────

/// Generic registry holding one or more named instances of a `Checkable` service.
///
/// Supports both flat (single-instance) and multi-instance configurations.
/// The resolution order for `resolve(project)` is:
/// 1. `project_map` lookup → instance name → client
/// 2. `"default"` instance (backward-compat flat configs)
/// 3. First instance in the map
pub struct ServiceRegistry<T: Checkable> {
    /// Instance name → client.
    instances: HashMap<String, T>,
    /// Project code → instance name (from config's `project_map`).
    project_map: HashMap<String, String>,
}

// Methods `new`, `get`, `iter`, `is_empty`, `len` are part of the public API
// and used in tests; the binary uses only `single`, `resolve`, and `default`.
#[allow(dead_code)]
impl<T: Checkable> ServiceRegistry<T> {
    /// Create a new registry with explicit instances and project map.
    pub fn new(instances: HashMap<String, T>, project_map: HashMap<String, String>) -> Self {
        Self {
            instances,
            project_map,
        }
    }

    /// Create a registry with a single `"default"` instance.
    pub fn single(instance: T) -> Self {
        let mut instances = HashMap::new();
        instances.insert("default".to_string(), instance);
        Self {
            instances,
            project_map: HashMap::new(),
        }
    }

    /// Resolve the correct client for a given project code.
    ///
    /// Resolution order:
    /// 1. `project_map` → instance name → client
    /// 2. `"default"` instance
    /// 3. First instance
    pub fn resolve(&self, project: &str) -> Option<&T> {
        // 1. project_map
        if let Some(instance_name) = self.project_map.get(project) {
            if let Some(client) = self.instances.get(instance_name) {
                return Some(client);
            }
        }
        // 2. "default"
        if let Some(client) = self.instances.get("default") {
            return Some(client);
        }
        // 3. first
        self.instances.values().next()
    }

    /// Direct instance lookup by name.
    pub fn get(&self, instance: &str) -> Option<&T> {
        self.instances.get(instance)
    }

    /// Return the default or first instance (for call sites without project context).
    pub fn default(&self) -> Option<&T> {
        self.instances
            .get("default")
            .or_else(|| self.instances.values().next())
    }

    /// Iterate all instances as `(instance_name, client)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.instances.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Returns true if the registry has no instances.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Number of configured instances.
    pub fn len(&self) -> usize {
        self.instances.len()
    }
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use nv_core::channel::Channel;
use nv_core::types::OutboundMessage;

use self::ado as ado_tools;
use self::discord as discord_tools;
use crate::aggregation;
use self::outlook as outlook_tools;
use self::calendar as calendar_tools;
use crate::claude::ToolDefinition;
use crate::memory::Memory;
use crate::nexus;
use crate::reminders::{self, ReminderStore};
use self::schedule as schedule_tools;
use self::teams as teams_tools;

// ── Dispatch timeouts ────────────────────────────────────────────────

/// Maximum execution budget for read-class tools (no state mutation).
const TOOL_TIMEOUT_READ: Duration = Duration::from_secs(30);

/// Maximum execution budget for write-class tools (state mutation / external calls).
const TOOL_TIMEOUT_WRITE: Duration = Duration::from_secs(60);

/// Register all available tool definitions for the Anthropic API.
///
/// Returns tool schemas in the Anthropic `tools` format.
/// Includes memory tools, Jira tools, and Nexus tools.
pub fn register_tools() -> Vec<ToolDefinition> {
    let mut tools = vec![
        ToolDefinition {
            name: "read_memory".into(),
            description: "Read a specific memory topic file. Returns the contents of the topic.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "The memory topic to read (e.g., 'tasks', 'preferences', 'project-notes')"
                    }
                },
                "required": ["topic"]
            }),
        },
        ToolDefinition {
            name: "search_memory".into(),
            description: "Search across all memory files for relevant information. Returns matching excerpts.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to match against memory contents"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "write_memory".into(),
            description: "Store information in a memory topic for future reference. Appends to the topic file.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "The memory topic to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    }
                },
                "required": ["topic", "content"]
            }),
        },
        ToolDefinition {
            name: "update_soul".into(),
            description: "Update Nova's soul/personality file (soul.md). Use sparingly — always notify the operator about what changed and why.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The full new content for soul.md"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "get_recent_messages".into(),
            description: "Get recent messages from the conversation history. Returns the last N messages formatted with timestamps and senders.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {
                        "type": "integer",
                        "description": "Number of recent messages to return (default: 20, max: 100)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "search_messages".into(),
            description: "Search past conversations using full-text search. Returns messages matching the query ranked by relevance.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (supports FTS5 syntax: AND, OR, NOT, phrases)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return (default 10, max 50)"
                    }
                },
                "required": ["query"]
            }),
        },
    ];

    // Add Azure DevOps tools
    tools.extend(ado_tools::ado_tool_definitions());

    // Add aggregation composite tools
    tools.extend(aggregation::aggregation_tool_definitions());

    // Add session lifecycle tools (start/stop CC sessions via TeamAgentDispatcher)
    tools.push(ToolDefinition {
        name: "start_session".into(),
        description: "Start a new Claude Code session on a project. Requires confirmation before execution. Example: start_session('oo', '/apply fix-chat-bugs')".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                },
                "command": {
                    "type": "string",
                    "description": "Command to run in the session (e.g. '/apply fix-chat-bugs', '/feature')"
                },
                "agent": {
                    "type": "string",
                    "description": "Target a specific agent by name instead of round-robin. Optional."
                }
            },
            "required": ["project", "command"]
        }),
    });
    tools.push(ToolDefinition {
        name: "stop_session".into(),
        description: "Stop a running CC session. Requires confirmation before execution. Use to kill runaway sessions.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to stop"
                }
            },
            "required": ["session_id"]
        }),
    });

    // Add schedule management tools
    tools.extend(schedule_tool_definitions());

    // Add Google Calendar read-only tools
    tools.extend(calendar_tool_definitions());

    // Add reminder tools (set, list, cancel)
    tools.extend(reminder_tool_definitions());

    // Add Microsoft Teams tools (channels, messages, send, presence)
    tools.extend(teams_tools::teams_tool_definitions());

    // Add Discord read tools (list_guilds, list_channels, read_messages)
    tools.extend(discord_tools::discord_tool_definitions());

    // Add Outlook email and calendar tools (delegated auth)
    tools.extend(outlook_tools::outlook_tool_definitions());

    // Add service diagnostics tool (must appear before general_tool_definitions removal)
    tools.push(ToolDefinition {
        name: "check_services".into(),
        description: "Run connectivity and credential probes against all configured services. \
            Returns a structured report showing which services are healthy, degraded, \
            unhealthy, or missing credentials. Use this proactively when a tool call \
            fails with an auth error, or to diagnose service connectivity issues.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "read_only": {
                    "type": "boolean",
                    "description": "If true, skip write probes and only run read connectivity checks. Default false."
                },
                "service": {
                    "type": "string",
                    "description": "Optional: check only the named service (partial match on service name, e.g. 'stripe', 'jira')."
                }
            },
            "required": []
        }),
    });

    // Add cross-channel routing tools (list_channels, send_to_channel)
    tools.extend(channels::channels_tool_definitions());

    // Self-assessment tool (no parameters)
    tools.push(ToolDefinition {
        name: "self_assessment_run".into(),
        description: "Run a weekly self-assessment analyzing Nova's performance over the past 7 days. Returns a summary report covering cold-start latency trends, tool error rates, usage patterns, and actionable suggestions.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    });

    tools
}

/// Execute a schedule management tool synchronously.
///
/// Handles `list_schedules`, `add_schedule`, `modify_schedule`, `remove_schedule`.
/// Called directly by the worker before delegating to `execute_tool_send` so the
/// `ScheduleStore` reference (wrapping `rusqlite::Connection`, a `!Send` type) does
/// not cross an await point.
///
/// Returns `None` if `name` is not a schedule tool (caller should fall through to
/// `execute_tool_send`).
pub fn execute_schedule_tool(
    name: &str,
    input: &serde_json::Value,
    store: &schedule_tools::ScheduleStore,
) -> Option<Result<ToolResult>> {
    match name {
        "list_schedules" => Some(list_schedules_impl(store, input)),
        "add_schedule" => Some(add_schedule_impl(store, input)),
        "modify_schedule" => Some(modify_schedule_impl(store, input)),
        "remove_schedule" => Some(remove_schedule_impl(store, input)),
        _ => None,
    }
}

fn list_schedules_impl(
    store: &schedule_tools::ScheduleStore,
    _input: &serde_json::Value,
) -> Result<ToolResult> {
    let user_schedules = store.list()?;
    let mut lines = vec!["Schedules:".to_string()];

    // Built-in schedules (hardcoded, always enabled)
    lines.push("- digest (built-in) — always enabled — runs at configured digest interval".to_string());
    lines.push("- memory-cleanup (built-in) — always enabled — runs on memory cleanup interval".to_string());

    // User schedules from SQLite
    if user_schedules.is_empty() {
        lines.push("(no user-created schedules)".to_string());
    } else {
        for s in &user_schedules {
            let status = if s.enabled { "enabled" } else { "paused" };
            let next = schedule_tools::describe_cron(&s.cron_expr);
            let last = s.last_run_at.as_deref().unwrap_or("never");
            lines.push(format!(
                "- {} ({}) — {} — {} — last run: {}",
                s.name, s.action, status, next, last
            ));
        }
    }

    Ok(ToolResult::Immediate(lines.join("\n")))
}

fn add_schedule_impl(
    store: &schedule_tools::ScheduleStore,
    input: &serde_json::Value,
) -> Result<ToolResult> {
    let name = input["name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'name' parameter"))?;
    let cron_expr = input["cron_expr"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'cron_expr' parameter"))?;
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'action' parameter"))?;
    let _channel = input["channel"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'channel' parameter"))?;

    // Validate name format
    schedule_tools::validate_schedule_name(name)?;

    // Reject reserved names
    if schedule_tools::RESERVED_NAMES.contains(&name) {
        anyhow::bail!("'{}' is a reserved name and cannot be used for user schedules", name);
    }

    // Validate cron expression
    schedule_tools::validate_cron_expr(cron_expr)?;

    // Reject duplicate names
    if store.get(name)?.is_some() {
        anyhow::bail!("a schedule named '{}' already exists", name);
    }

    let description = format!("Add schedule '{}': {} ({})", name, cron_expr, action);
    Ok(ToolResult::PendingAction {
        description,
        action_type: nv_core::types::ActionType::ScheduleAdd,
        payload: input.clone(),
    })
}

fn modify_schedule_impl(
    store: &schedule_tools::ScheduleStore,
    input: &serde_json::Value,
) -> Result<ToolResult> {
    let name = input["name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'name' parameter"))?;

    // Reject built-in names
    if schedule_tools::RESERVED_NAMES.contains(&name) {
        anyhow::bail!("'{}' is a built-in schedule and cannot be modified", name);
    }

    // At least one of cron_expr or enabled must be provided
    let has_cron = input.get("cron_expr").and_then(|v| v.as_str()).is_some();
    let has_enabled = input.get("enabled").and_then(|v| v.as_bool()).is_some();
    if !has_cron && !has_enabled {
        anyhow::bail!("at least one of 'cron_expr' or 'enabled' must be provided");
    }

    // Validate cron expression if provided
    if has_cron {
        let expr = input["cron_expr"].as_str().unwrap();
        schedule_tools::validate_cron_expr(expr)?;
    }

    // Validate schedule exists
    if store.get(name)?.is_none() {
        anyhow::bail!("schedule '{}' not found", name);
    }

    let description = format!("Modify schedule '{}'", name);
    Ok(ToolResult::PendingAction {
        description,
        action_type: nv_core::types::ActionType::ScheduleModify,
        payload: input.clone(),
    })
}

fn remove_schedule_impl(
    store: &schedule_tools::ScheduleStore,
    input: &serde_json::Value,
) -> Result<ToolResult> {
    let name = input["name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'name' parameter"))?;

    // Reject built-in names
    if schedule_tools::RESERVED_NAMES.contains(&name) {
        anyhow::bail!("'{}' is a built-in schedule and cannot be removed", name);
    }

    // Validate schedule exists
    if store.get(name)?.is_none() {
        anyhow::bail!("schedule '{}' not found", name);
    }

    let description = format!("Remove schedule '{}'", name);
    Ok(ToolResult::PendingAction {
        description,
        action_type: nv_core::types::ActionType::ScheduleRemove,
        payload: input.clone(),
    })
}


/// Tool definitions for schedule management (list, add, modify, remove).
fn schedule_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_schedules".into(),
            description: "List all recurring schedules (built-in and user-created) with next fire time.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "add_schedule".into(),
            description: "Create a new user-defined recurring schedule. Requires confirmation. Name must be unique, lowercase alphanumeric + hyphens. Action is one of: digest, health_check, reminder.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Unique schedule label (lowercase, alphanumeric + hyphens, e.g. 'morning-health')"
                    },
                    "cron_expr": {
                        "type": "string",
                        "description": "Standard 5-field cron expression (min hr dom mon dow, e.g. '0 8 * * 1-5')"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["digest", "health_check", "reminder"],
                        "description": "What to run when the schedule fires"
                    },
                    "channel": {
                        "type": "string",
                        "description": "Originating channel name (e.g. 'telegram')"
                    }
                },
                "required": ["name", "cron_expr", "action", "channel"]
            }),
        },
        ToolDefinition {
            name: "modify_schedule".into(),
            description: "Update an existing user schedule. At least one of cron_expr or enabled must be provided. Requires confirmation. Cannot modify built-in schedules.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Schedule name to modify"
                    },
                    "cron_expr": {
                        "type": "string",
                        "description": "New 5-field cron expression (optional)"
                    },
                    "enabled": {
                        "type": "boolean",
                        "description": "Set to false to pause, true to resume (optional)"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "remove_schedule".into(),
            description: "Delete a user-defined schedule by name. Requires confirmation. Cannot remove built-in schedules.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Schedule name to delete"
                    }
                },
                "required": ["name"]
            }),
        },
    ]
}

/// Tool definitions for Google Calendar read-only queries.
fn calendar_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "calendar_today".into(),
            description: "Get today's calendar events. Returns a formatted schedule for the current day.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "calendar_upcoming".into(),
            description: "Get calendar events for the next N days (default 7, max 30). Returns events grouped by day.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "days": {
                        "type": "integer",
                        "description": "Number of days to look ahead (default: 7, max: 30)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "calendar_next".into(),
            description: "Get the single next upcoming calendar event. Quick check for what's on deck.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

/// Tool definitions for the reminders system.
fn reminder_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "set_reminder".into(),
            description: "Set a reminder that will fire as a message at the specified time. Use for 'remind me to...' requests.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "What to remind about (e.g. 'check the CT deploy')"
                    },
                    "due_at": {
                        "type": "string",
                        "description": "When to fire — ISO 8601 datetime OR relative like '2h', '30m', 'tomorrow 9am', 'next Monday'"
                    },
                    "channel": {
                        "type": "string",
                        "description": "Channel to send reminder to (default: channel the request came from)"
                    }
                },
                "required": ["message", "due_at"]
            }),
        },
        ToolDefinition {
            name: "list_reminders".into(),
            description: "List all active (unfired, uncancelled) reminders with their IDs and due times.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "cancel_reminder".into(),
            description: "Cancel an active reminder by its ID. Returns whether the cancellation succeeded.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "The reminder ID to cancel (from list_reminders)"
                    }
                },
                "required": ["id"]
            }),
        },
    ]
}


/// Bootstrap-only tools — only write_memory, complete_bootstrap, and update_soul.
/// Used during first-run to prevent Claude from searching Jira/memory
/// instead of focusing on the onboarding conversation.
pub fn register_bootstrap_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "write_memory".into(),
            description: "Write content to a memory file (identity.md, user.md, soul.md, or any topic). Used during bootstrap to save configuration.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "The topic/filename to write (e.g. 'identity', 'user', 'soul')"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    }
                },
                "required": ["topic", "content"]
            }),
        },
        ToolDefinition {
            name: "update_soul".into(),
            description: "Update Nova's soul/personality file (soul.md).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The full new content for soul.md"
                    }
                },
                "required": ["content"]
            }),
        },
    ]
}

/// Result of executing a tool — either an immediate result or a
/// pending action that requires Telegram confirmation.
#[derive(Debug)]
pub enum ToolResult {
    /// Immediate text result to return to Claude.
    Immediate(String),
    /// A Jira write operation that needs confirmation before executing.
    PendingAction {
        description: String,
        action_type: nv_core::types::ActionType,
        payload: serde_json::Value,
    },
}

/// Execute a reminder tool synchronously (same pattern as schedule tools).
///
/// Handles `set_reminder`, `list_reminders`, `cancel_reminder`.
/// Returns `None` if `name` is not a reminder tool (caller falls through to
/// `execute_tool_send`).
pub fn execute_reminder_tool(
    name: &str,
    input: &serde_json::Value,
    store: &ReminderStore,
    trigger_channel: &str,
    timezone: &str,
) -> Option<Result<ToolResult>> {
    match name {
        "set_reminder" => Some(set_reminder_impl(store, input, trigger_channel, timezone)),
        "list_reminders" => Some(list_reminders_impl(store, timezone)),
        "cancel_reminder" => Some(cancel_reminder_impl(store, input)),
        _ => None,
    }
}

fn set_reminder_impl(
    store: &ReminderStore,
    input: &serde_json::Value,
    trigger_channel: &str,
    timezone: &str,
) -> Result<ToolResult> {
    let message = input["message"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'message' parameter"))?;
    let due_at_str = input["due_at"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'due_at' parameter"))?;
    let channel = input["channel"]
        .as_str()
        .unwrap_or(trigger_channel);

    let due_at = reminders::parse_relative_time(due_at_str, timezone)?;
    let id = store.create_reminder(message, &due_at, channel, None)?;

    let due_display = due_at.format("%Y-%m-%d %H:%M UTC").to_string();
    Ok(ToolResult::Immediate(format!(
        "Reminder set (ID: {id}). I'll remind you to '{message}' at {due_display} via {channel}."
    )))
}

fn list_reminders_impl(store: &ReminderStore, timezone: &str) -> Result<ToolResult> {
    let reminders = store.list_active_reminders()?;
    Ok(ToolResult::Immediate(reminders::format_reminders_list(&reminders, timezone)))
}

fn cancel_reminder_impl(store: &ReminderStore, input: &serde_json::Value) -> Result<ToolResult> {
    let id = input["id"]
        .as_i64()
        .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
    let found = store.cancel_reminder(id)?;
    if found {
        Ok(ToolResult::Immediate(format!("Reminder {id} cancelled.")))
    } else {
        Ok(ToolResult::Immediate(format!(
            "Reminder {id} not found or already delivered/cancelled."
        )))
    }
}

/// Execute a tool without access to `MessageStore`, `ScheduleStore`, or `ReminderStore`
/// (Send-safe).
///
/// This variant avoids referencing stores that wrap `rusqlite::Connection` (a `!Send` type)
/// so the resulting future can be used with `tokio::spawn`. The `get_recent_messages`,
/// Pre-built service client registries, threaded through `execute_tool_send`
/// so workers reuse persistent connections instead of re-reading env vars on
/// every tool call.
///
/// Each field is `Option` — `None` means "not configured / fall back to
/// `from_env()` inline".
// Some fields not yet consumed in execute_tool_send until the service-registry
// migration is complete for all tool handlers.
#[allow(dead_code)]
pub struct ServiceRegistries<'a> {
    pub stripe: Option<&'a ServiceRegistry<stripe::StripeClient>>,
    pub vercel: Option<&'a ServiceRegistry<vercel::VercelClient>>,
    pub sentry: Option<&'a ServiceRegistry<sentry::SentryClient>>,
    pub resend: Option<&'a ServiceRegistry<resend::ResendClient>>,
    pub ha: Option<&'a ServiceRegistry<ha::HAClient>>,
    pub upstash: Option<&'a ServiceRegistry<upstash::UpstashClient>>,
    pub ado: Option<&'a ServiceRegistry<ado::AdoClient>>,
    pub cloudflare: Option<&'a ServiceRegistry<cloudflare::CloudflareClient>>,
    pub doppler: Option<&'a ServiceRegistry<doppler::DopplerClient>>,
    /// Cached Teams client — avoids rebuilding OAuth token state on every tool call.
    pub teams: Option<&'a crate::channels::teams::client::TeamsClient>,
}

/// Returns `true` for tools that mutate state or send external messages.
/// These get the longer `TOOL_TIMEOUT_WRITE` budget (60 s).
fn is_write_tool(name: &str) -> bool {
    matches!(
        name,
        "write_memory"
            | "start_session"
            | "stop_session"
            | "send_to_channel"
            | "teams_send"
            | "update_soul"
    )
}

/// `search_messages`, schedule tools, and reminder tools must be handled by the caller
/// before delegating here.
#[allow(clippy::too_many_arguments, dead_code)]
pub async fn execute_tool_send(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_registry: Option<&jira::JiraRegistry>,
    project_registry: &HashMap<String, PathBuf>,
    channels: &HashMap<String, Arc<dyn Channel>>,
    calendar_credentials: Option<&str>,
    calendar_id: &str,
    service_registries: &ServiceRegistries<'_>,
) -> Result<ToolResult> {
    execute_tool_send_with_backend(
        name, input, memory, jira_registry, None,
        project_registry, channels, calendar_credentials, calendar_id, service_registries,
    )
    .await
}

/// Variant of `execute_tool_send` that accepts an optional `NexusBackend`.
///
/// When `nexus_backend` is `Some`, all nexus tool calls are routed through it
/// (team-agents dispatcher). When `None`, nexus tools return "not configured".
#[allow(clippy::too_many_arguments)]
pub async fn execute_tool_send_with_backend(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_backend: Option<&nexus::backend::NexusBackend>,
    project_registry: &HashMap<String, PathBuf>,
    channels: &HashMap<String, Arc<dyn Channel>>,
    calendar_credentials: Option<&str>,
    calendar_id: &str,
    service_registries: &ServiceRegistries<'_>,
) -> Result<ToolResult> {
    let budget = if is_write_tool(name) {
        TOOL_TIMEOUT_WRITE
    } else {
        TOOL_TIMEOUT_READ
    };
    let dispatch = async {
        match name {
        "read_memory" => {
            let topic = input["topic"].as_str().ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            memory.read(topic).map(ToolResult::Immediate)
        }
        "search_memory" => {
            let query = input["query"].as_str().ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            memory.search(query).map(ToolResult::Immediate)
        }
        "write_memory" => {
            let topic = input["topic"].as_str().ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            let content = input["content"].as_str().ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            memory.write(topic, content).map(ToolResult::Immediate)
        }
        "update_soul" => {
            let content = input["content"].as_str().ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home).join(".nv").join("soul.md");
            std::fs::write(&path, content).map_err(|e| anyhow!("failed to write soul.md: {e}"))?;
            Ok(ToolResult::Immediate("Soul updated. Notification sent to Leo.".into()))
        }
        "get_recent_messages" => Err(anyhow!("get_recent_messages must be handled by the worker directly")),

        // ── Session Lifecycle ──────────────────────────────────────
        "start_session" => {
            // Validate that a backend is configured before queuing the action
            if nexus_backend.is_none() {
                anyhow::bail!("Team agents not configured");
            }
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'command' parameter"))?;
            let agent = input["agent"].as_str();
            let description = if let Some(agent_name) = agent {
                format!(
                    "Start CC session on {} via {}: `{}`",
                    project.to_uppercase(),
                    agent_name,
                    command
                )
            } else {
                format!(
                    "Start CC session on {}: `{}`",
                    project.to_uppercase(),
                    command
                )
            };
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::NexusStartSession,
                payload: input.clone(),
            })
        }
        "stop_session" => {
            // Validate that a backend is configured before queuing the action
            if nexus_backend.is_none() {
                anyhow::bail!("Team agents not configured");
            }
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'session_id' parameter"))?;
            let description = format!("Stop session {session_id}");
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::NexusStopSession,
                payload: input.clone(),
            })
        }

        // ── Aggregation Tools ────────────────────────────────────
        "project_health" => {
            let code = input["code"].as_str().ok_or_else(|| anyhow!("missing 'code' parameter"))?;
            let jira_client = jira_registry.and_then(|r| r.resolve(code));
            let output = aggregation::project_health(code, jira_client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "homelab_status" => {
            let output = aggregation::homelab_status().await?;
            Ok(ToolResult::Immediate(output))
        }
        "financial_summary" => {
            let output = aggregation::financial_summary().await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Azure DevOps Tools ───────────────────────────────────────
        "ado_projects" => {
            let output = ado_tools::ado_projects().await?;
            Ok(ToolResult::Immediate(output))
        }
        "ado_pipelines" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let output = ado_tools::ado_pipelines(project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "ado_builds" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let pipeline_id = input["pipeline_id"]
                .as_u64()
                .ok_or_else(|| anyhow!("missing 'pipeline_id' parameter"))? as u32;
            let output = ado_tools::ado_builds(project, pipeline_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_ado_work_items" => {
            let project = input["project"].as_str();
            let assigned_to = input["assigned_to"].as_str().unwrap_or("@Me");
            let state = input["state"].as_str().unwrap_or("active");
            let limit = input["limit"].as_u64().unwrap_or(20).min(50) as usize;
            let output = ado_tools::query_ado_work_items(project, assigned_to, state, limit).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Outlook Tools ────────────────────────────────────────────
        "read_outlook_inbox" => {
            let folder = input["folder"].as_str();
            let count = input["count"].as_u64().unwrap_or(10).min(25) as u32;
            let unread_only = input["unread_only"].as_bool().unwrap_or(false);
            let output = outlook_tools::read_inbox(folder, count, unread_only).await?;
            Ok(ToolResult::Immediate(output))
        }
        "read_outlook_calendar" => {
            let days_ahead = input["days_ahead"].as_u64().unwrap_or(1).min(30) as u32;
            let max_events = input["max_events"].as_u64().unwrap_or(10).min(25) as u32;
            let output = outlook_tools::read_calendar(days_ahead, max_events).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Cross-Channel Routing ─────────────────────────────────
        "list_channels" => {
            let output = channels::list_channels(channels)?;
            let channel_count = channels.len();
            tracing::info!(channels = channel_count, "tool:list_channels invoked");
            Ok(ToolResult::Immediate(output))
        }
        "send_to_channel" => {
            let channel_name = input["channel"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'channel' parameter"))?;
            let message = input["message"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'message' parameter"))?;

            let request = channels::send_to_channel(channels, channel_name, message)?;

            tracing::info!(
                channel = channel_name,
                msg_len = message.len(),
                "tool:send_to_channel queued pending action"
            );

            Ok(ToolResult::PendingAction {
                description: request.description,
                action_type: nv_core::types::ActionType::ChannelSend,
                payload: request.payload,
            })
        }

        // ── Calendar Tools ────────────────────────────────────────────
        "calendar_today" => {
            let client = calendar_tools::build_client(calendar_credentials, calendar_id)
                .map_err(|e| anyhow!("{e}"))?;
            let output = calendar_tools::calendar_today(&client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "calendar_upcoming" => {
            let client = calendar_tools::build_client(calendar_credentials, calendar_id)
                .map_err(|e| anyhow!("{e}"))?;
            let days = input["days"].as_u64().map(|d| d as u32);
            let output = calendar_tools::calendar_upcoming(&client, days).await?;
            Ok(ToolResult::Immediate(output))
        }
        "calendar_next" => {
            let client = calendar_tools::build_client(calendar_credentials, calendar_id)
                .map_err(|e| anyhow!("{e}"))?;
            let output = calendar_tools::calendar_next(&client).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Teams Tools ───────────────────────────────────────────────
        "teams_channels" => {
            let _owned_teams;
            let client: &crate::channels::teams::client::TeamsClient =
                if let Some(c) = service_registries.teams {
                    c
                } else {
                    let secrets = nv_core::config::Secrets::from_env()?;
                    _owned_teams = teams_tools::build_teams_client(&secrets, None)?;
                    &_owned_teams
                };
            let env_team_id = std::env::var("NV_TEAMS_TEAM_ID").ok();
            let team_id = input["team_id"]
                .as_str()
                .or(env_team_id.as_deref())
                .ok_or_else(|| anyhow!("team_id is required. Pass it as a parameter or set NV_TEAMS_TEAM_ID env var."))?;
            let output = teams_tools::teams_channels(client, team_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "teams_messages" => {
            let _owned_teams;
            let client: &crate::channels::teams::client::TeamsClient =
                if let Some(c) = service_registries.teams {
                    c
                } else {
                    let secrets = nv_core::config::Secrets::from_env()?;
                    _owned_teams = teams_tools::build_teams_client(&secrets, None)?;
                    &_owned_teams
                };
            let env_team_id = std::env::var("NV_TEAMS_TEAM_ID").ok();
            let team_id = input["team_id"]
                .as_str()
                .or(env_team_id.as_deref())
                .ok_or_else(|| anyhow!("team_id is required. Pass it as a parameter or set NV_TEAMS_TEAM_ID env var."))?;
            let channel_id = input["channel_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'channel_id' parameter"))?;
            let output = teams_tools::teams_messages(client, team_id, channel_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "teams_send" => {
            let env_team_id = std::env::var("NV_TEAMS_TEAM_ID").ok();
            let team_id = input["team_id"]
                .as_str()
                .or(env_team_id.as_deref())
                .ok_or_else(|| anyhow!("team_id is required. Pass it as a parameter or set NV_TEAMS_TEAM_ID env var."))?;
            let channel_id = input["channel_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'channel_id' parameter"))?;
            let message = input["message"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'message' parameter"))?;
            if message.trim().is_empty() {
                anyhow::bail!("message must be non-empty");
            }
            let preview = if message.len() > 60 {
                format!("{}…", &message[..60])
            } else {
                message.to_string()
            };
            let description = format!("Send to Teams #{channel_id}: \"{preview}\"");
            let payload = serde_json::json!({
                "channel": "teams",
                "message": message,
                "recipient": format!("{team_id}:{channel_id}"),
            });
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::ChannelSend,
                payload,
            })
        }
        "teams_presence" => {
            let _owned_teams;
            let client: &crate::channels::teams::client::TeamsClient =
                if let Some(c) = service_registries.teams {
                    c
                } else {
                    let secrets = nv_core::config::Secrets::from_env()?;
                    _owned_teams = teams_tools::build_teams_client(&secrets, None)?;
                    &_owned_teams
                };
            let user = input["user"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'user' parameter"))?;
            let output = teams_tools::teams_presence(client, user).await?;
            Ok(ToolResult::Immediate(output))
        }
        "teams_list_chats" => {
            let _owned_teams;
            let client: &crate::channels::teams::client::TeamsClient =
                if let Some(c) = service_registries.teams {
                    c
                } else {
                    let secrets = nv_core::config::Secrets::from_env()?;
                    _owned_teams = teams_tools::build_teams_client(&secrets, None)?;
                    &_owned_teams
                };
            let limit = input["limit"].as_u64().unwrap_or(20).min(50) as usize;
            let output = teams_tools::teams_list_chats(client, limit).await?;
            Ok(ToolResult::Immediate(output))
        }
        "teams_read_chat" => {
            let _owned_teams;
            let client: &crate::channels::teams::client::TeamsClient =
                if let Some(c) = service_registries.teams {
                    c
                } else {
                    let secrets = nv_core::config::Secrets::from_env()?;
                    _owned_teams = teams_tools::build_teams_client(&secrets, None)?;
                    &_owned_teams
                };
            let chat_id = input["chat_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'chat_id' parameter"))?;
            let limit = input["limit"].as_u64().unwrap_or(20).min(50) as usize;
            let output = teams_tools::teams_read_chat(client, chat_id, limit).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Discord Tools ─────────────────────────────────────────────
        "discord_list_guilds" => {
            let secrets = nv_core::config::Secrets::from_env()?;
            let client = discord_tools::build_discord_client(&secrets)?;
            let output = discord_tools::discord_list_guilds(&client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "discord_list_channels" => {
            let guild_id = input["guild_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'guild_id' parameter"))?;
            let secrets = nv_core::config::Secrets::from_env()?;
            let client = discord_tools::build_discord_client(&secrets)?;
            let output = discord_tools::discord_list_channels(&client, guild_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "discord_read_messages" => {
            let channel_id = input["channel_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'channel_id' parameter"))?;
            let limit = input["limit"].as_u64().unwrap_or(20).min(50) as usize;
            let secrets = nv_core::config::Secrets::from_env()?;
            let client = discord_tools::build_discord_client(&secrets)?;
            let output = discord_tools::discord_read_messages(&client, channel_id, limit).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Service Diagnostics ───────────────────────────────────────
        "check_services" => {
            let read_only = input["read_only"].as_bool().unwrap_or(false);
            let service_filter = input["service"].as_str();
            let include_write = !read_only;

            // Build owned Checkable instances from environment variables.
            // The service registries already hold live clients but they yield
            // &T references; we build fresh clients via from_env() here so
            // check_all() can take &dyn Checkable with proper lifetimes.
            let mut owned: Vec<Box<dyn Checkable>> = Vec::new();

            macro_rules! push_env {
                ($ctor:expr, $missing_name:expr, $missing_var:expr) => {
                    match $ctor {
                        Ok(c) => owned.push(Box::new(c)),
                        Err(_) => owned.push(Box::new(check::MissingService::new(
                            $missing_name,
                            $missing_var,
                        ))),
                    }
                };
            }

            push_env!(stripe::StripeClient::from_env(), "stripe", "STRIPE_SECRET_KEY");
            push_env!(vercel::VercelClient::from_env(), "vercel", "VERCEL_TOKEN");
            push_env!(sentry::SentryClient::from_env(), "sentry", "SENTRY_AUTH_TOKEN");
            push_env!(resend::ResendClient::from_env(), "resend", "RESEND_API_KEY");
            push_env!(ha::HAClient::from_env(), "ha", "HA_TOKEN");
            push_env!(upstash::UpstashClient::from_env(), "upstash", "UPSTASH_REDIS_REST_URL");
            push_env!(ado::AdoClient::from_env(), "ado", "ADO_PAT");
            push_env!(cloudflare::CloudflareClient::from_env(), "cloudflare", "CLOUDFLARE_API_TOKEN");
            push_env!(doppler::DopplerClient::from_env(), "doppler", "DOPPLER_API_TOKEN");

            // Zero-arg constructors (check_read resolves credentials internally)
            owned.push(Box::new(posthog::PosthogClient));
            owned.push(Box::new(github::GithubClient));
            owned.push(Box::new(docker::DockerClient));
            owned.push(Box::new(plaid::PlaidClient));
            owned.push(Box::new(teams::TeamsCheck));
            owned.push(Box::new(jira::JiraCheck));

            // Calendar uses env-based construction
            push_env!(calendar::from_env(), "calendar", "GOOGLE_CALENDAR_CREDENTIALS");

            // Neon uses a project-specific URL; use "default" project as the probe target
            owned.push(Box::new(neon::NeonClient::new("default")));

            // Apply optional service filter
            let filtered: Vec<&dyn Checkable> = if let Some(filter) = service_filter {
                owned
                    .iter()
                    .filter(|s| s.name().contains(filter))
                    .map(|s| s.as_ref())
                    .collect()
            } else {
                owned.iter().map(|s| s.as_ref()).collect()
            };

            let report = check::check_all(&filtered, include_write).await;
            let json_value = check::format_json(&report);
            Ok(ToolResult::Immediate(
                serde_json::to_string_pretty(&json_value)
                    .unwrap_or_else(|e| format!("serialization error: {e}")),
            ))
        }

        _ => Err(anyhow!("unknown tool: {name}")),
        }
    };
    match tokio::time::timeout(budget, dispatch).await {
        Ok(result) => result,
        Err(_elapsed) => Ok(ToolResult::Immediate(format!(
            "Tool timed out after {}s",
            budget.as_secs()
        ))),
    }
}




/// Execute a confirmed Jira pending action via the registry.
///
/// Called when the user taps "Approve" on a Telegram inline keyboard.
/// The registry routes to the correct Jira instance based on the project/issue key
/// in the payload.
#[allow(dead_code)]
pub async fn execute_jira_action(
    jira_registry: &jira::JiraRegistry,
    action_type: &nv_core::types::ActionType,
    payload: &serde_json::Value,
) -> Result<String> {
    match action_type {
        nv_core::types::ActionType::JiraCreate => {
            let project = payload["project"].as_str().unwrap_or("");
            let client = jira_registry
                .resolve(project)
                .ok_or_else(|| anyhow!("Jira not configured for project {project}"))?;
            let params: jira::JiraCreateParams = serde_json::from_value(payload.clone())
                .map_err(|e| anyhow!("invalid jira_create payload: {e}"))?;
            let created = client.create_issue(&params).await?;
            Ok(format!("Created {}: {}", created.key, params.title))
        }
        nv_core::types::ActionType::JiraTransition => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in transition payload"))?;
            let transition_name = payload["transition_name"]
                .as_str()
                .ok_or_else(|| anyhow!("missing transition_name in transition payload"))?;
            let client = jira_registry
                .resolve_from_issue_key(issue_key)
                .ok_or_else(|| anyhow!("Jira not configured for issue {issue_key}"))?;
            client.transition_issue(issue_key, transition_name).await?;
            Ok(format!("Transitioned {issue_key} to {transition_name}"))
        }
        nv_core::types::ActionType::JiraAssign => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in assign payload"))?;
            let assignee = payload["assignee_account_id"]
                .as_str()
                .or_else(|| payload["assignee"].as_str())
                .ok_or_else(|| anyhow!("missing assignee in assign payload"))?;
            let client = jira_registry
                .resolve_from_issue_key(issue_key)
                .ok_or_else(|| anyhow!("Jira not configured for issue {issue_key}"))?;
            client.assign_issue(issue_key, assignee).await?;
            Ok(format!("Assigned {issue_key} to {assignee}"))
        }
        nv_core::types::ActionType::JiraComment => {
            let issue_key = payload["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing issue_key in comment payload"))?;
            let body = payload["body"]
                .as_str()
                .ok_or_else(|| anyhow!("missing body in comment payload"))?;
            let client = jira_registry
                .resolve_from_issue_key(issue_key)
                .ok_or_else(|| anyhow!("Jira not configured for issue {issue_key}"))?;
            let comment = client.add_comment(issue_key, body).await?;
            Ok(format!("Added comment {} to {issue_key}", comment.id))
        }
        _ => Err(anyhow!("Not a Jira action type: {action_type:?}")),
    }
}

/// Execute a confirmed ChannelSend pending action.
///
/// Called when the user taps "Approve" on the Telegram inline keyboard for
/// a `send_to_channel` tool invocation. Deserializes the payload, looks up
/// the target channel, and calls `Channel::send_message`.
pub async fn execute_channel_send(
    channels: &HashMap<String, Arc<dyn Channel>>,
    payload: &serde_json::Value,
) -> Result<String> {
    let channel_name = payload["channel"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'channel' in payload"))?;
    let message = payload["message"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'message' in payload"))?;
    let recipient = payload["recipient"].as_str().map(String::from);

    let channel = channels
        .get(channel_name)
        .ok_or_else(|| anyhow!("Channel '{}' not found", channel_name))?;

    channel
        .send_message(OutboundMessage {
            channel: channel_name.to_string(),
            content: message.to_string(),
            reply_to: recipient,
            keyboard: None,
        })
        .await?;

    Ok(format!("Message sent to {channel_name}"))
}

// ── Jira Validation Helpers ──────────────────────────────────────────

/// Validate that a string is a valid Jira project KEY.
///
/// Rules: 2-10 uppercase alphanumeric characters, starting with a letter.
/// Matches `^[A-Z][A-Z0-9]{1,9}$`.
pub fn validate_jira_project_key(key: &str) -> Result<()> {
    if key.is_empty() {
        anyhow::bail!(
            "Invalid project KEY '{}'. Must be 2-10 uppercase letters/digits starting with a letter (e.g., OO, TC, MV).",
            key
        );
    }
    let bytes = key.as_bytes();
    // Must start with uppercase letter
    if !bytes[0].is_ascii_uppercase() {
        anyhow::bail!(
            "Invalid project KEY '{}'. Must be 2-10 uppercase letters/digits starting with a letter (e.g., OO, TC, MV).",
            key
        );
    }
    // Length 2-10
    if key.len() < 2 || key.len() > 10 {
        anyhow::bail!(
            "Invalid project KEY '{}'. Must be 2-10 uppercase letters/digits starting with a letter (e.g., OO, TC, MV).",
            key
        );
    }
    // All chars must be uppercase alphanumeric
    if !bytes.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        anyhow::bail!(
            "Invalid project KEY '{}'. Must be 2-10 uppercase letters/digits starting with a letter (e.g., OO, TC, MV).",
            key
        );
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Memory) {
        let dir = TempDir::new().unwrap();
        let memory = Memory::new(dir.path());
        memory.init().unwrap();
        (dir, memory)
    }

    fn empty_registry() -> HashMap<String, PathBuf> {
        HashMap::new()
    }

    fn empty_channels() -> HashMap<String, Arc<dyn Channel>> {
        HashMap::new()
    }

    fn empty_service_registries() -> ServiceRegistries<'static> {
        ServiceRegistries {
            stripe: None,
            vercel: None,
            sentry: None,
            resend: None,
            ha: None,
            upstash: None,
            ado: None,
            cloudflare: None,
            doppler: None,
            teams: None,
        }
    }

    #[test]
    fn register_tools_returns_expected_count() {
        let tools = register_tools();
        // 3 memory (read, write, search) + 1 update_soul + 2 messages (get_recent + search)
        // + 4 ado + 3 aggregation
        // + 2 session lifecycle (start_session, stop_session)
        // + 4 schedule (list_schedules, add_schedule, modify_schedule, remove_schedule)
        // + 3 calendar (calendar_today, calendar_upcoming, calendar_next)
        // + 3 reminders (set_reminder, list_reminders, cancel_reminder)
        // + 1 check_services
        // + 2 cross-channel (list_channels, send_to_channel)
        // + 1 self-assessment (self_assessment_run)
        // + 6 teams (teams_channels, teams_messages, teams_send, teams_presence,
        //       teams_list_chats, teams_read_chat)
        // + 3 discord (discord_list_guilds, discord_list_channels, discord_read_messages)
        // + 2 outlook (read_outlook_inbox, read_outlook_calendar)
        // = 40
        assert_eq!(tools.len(), 40);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        // Core memory tools
        assert!(names.contains(&"read_memory"));
        assert!(names.contains(&"search_memory"));
        assert!(names.contains(&"write_memory"));
        assert!(names.contains(&"update_soul"));
        assert!(names.contains(&"get_recent_messages"));
        assert!(names.contains(&"search_messages"));
        // Azure DevOps tools
        assert!(names.contains(&"ado_projects"));
        assert!(names.contains(&"ado_pipelines"));
        assert!(names.contains(&"ado_builds"));
        assert!(names.contains(&"query_ado_work_items"));
        // Aggregation tools
        assert!(names.contains(&"project_health"));
        assert!(names.contains(&"homelab_status"));
        assert!(names.contains(&"financial_summary"));
        // Session lifecycle tools
        assert!(names.contains(&"start_session"));
        assert!(names.contains(&"stop_session"));
        // Schedule tools
        assert!(names.contains(&"list_schedules"));
        assert!(names.contains(&"add_schedule"));
        assert!(names.contains(&"modify_schedule"));
        assert!(names.contains(&"remove_schedule"));
        // Calendar tools
        assert!(names.contains(&"calendar_today"));
        assert!(names.contains(&"calendar_upcoming"));
        assert!(names.contains(&"calendar_next"));
        // Reminder tools
        assert!(names.contains(&"set_reminder"));
        assert!(names.contains(&"list_reminders"));
        assert!(names.contains(&"cancel_reminder"));
        // Service diagnostics
        assert!(names.contains(&"check_services"));
        // Cross-channel routing tools
        assert!(names.contains(&"list_channels"));
        assert!(names.contains(&"send_to_channel"));
        // Self-assessment
        assert!(names.contains(&"self_assessment_run"));
        // Teams tools
        assert!(names.contains(&"teams_channels"));
        assert!(names.contains(&"teams_messages"));
        assert!(names.contains(&"teams_send"));
        assert!(names.contains(&"teams_presence"));
        assert!(names.contains(&"teams_list_chats"));
        assert!(names.contains(&"teams_read_chat"));
        // Discord tools
        assert!(names.contains(&"discord_list_guilds"));
        assert!(names.contains(&"discord_list_channels"));
        assert!(names.contains(&"discord_read_messages"));
        // Outlook tools
        assert!(names.contains(&"read_outlook_inbox"));
        assert!(names.contains(&"read_outlook_calendar"));
    }

    #[test]
    fn tool_schemas_have_required_fields() {
        let tools = register_tools();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn read_memory_schema_requires_topic() {
        let tools = register_tools();
        let rm = tools.iter().find(|t| t.name == "read_memory").unwrap();
        let required = rm.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("topic")));
    }

    #[test]
    fn write_memory_schema_requires_topic_and_content() {
        let tools = register_tools();
        let wm = tools.iter().find(|t| t.name == "write_memory").unwrap();
        let required = wm.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("topic")));
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
    }

    #[tokio::test]
    async fn execute_read_memory_returns_content() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "read_memory",
            &serde_json::json!({"topic": "tasks"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("Tasks")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_read_memory_nonexistent() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "read_memory",
            &serde_json::json!({"topic": "nonexistent"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("No memory file found")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_search_memory_finds_content() {
        let (_dir, memory) = setup();
        memory.write("decisions", "Stripe fee is 5%").unwrap();

        let result = execute_tool_send(
            "search_memory",
            &serde_json::json!({"query": "Stripe"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Stripe"));
                assert!(s.contains("Found matches"));
            }
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_search_memory_no_results() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "search_memory",
            &serde_json::json!({"query": "xyznonexistent"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("No matches found")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_write_memory_creates_topic() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "write_memory",
            &serde_json::json!({"topic": "notes", "content": "hello world"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Created new memory topic"));
                assert!(s.contains("notes"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify it was written
        let read_result = execute_tool_send(
            "read_memory",
            &serde_json::json!({"topic": "notes"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match read_result {
            ToolResult::Immediate(s) => assert!(s.contains("hello world")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_jira_search_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "jira_search",
            &serde_json::json!({"jql": "project = NV"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Jira not configured"));
    }

    fn make_test_registry() -> jira::JiraRegistry {
        use nv_core::config::{JiraConfig, JiraInstanceConfig, Secrets};
        use std::collections::HashMap;
        let cfg = JiraConfig::Flat(JiraInstanceConfig {
            instance: "test.atlassian.net".to_string(),
            default_project: "OO".to_string(),
            webhook_secret: None,
        });
        let secrets = Secrets {
            anthropic_api_key: None,
            telegram_bot_token: None,
            discord_bot_token: None,
            bluebubbles_password: None,
            ms_graph_client_id: None,
            ms_graph_client_secret: None,
            ms_graph_tenant_id: None,
            jira_api_token: Some("fake-token".to_string()),
            jira_username: Some("test@test.com".to_string()),
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
            google_calendar_credentials: None,
        };
        jira::JiraRegistry::new(&cfg, &secrets).unwrap().unwrap()
    }

    #[tokio::test]
    async fn execute_jira_create_returns_pending_action() {
        let (_dir, memory) = setup();
        let registry = make_test_registry();
        let result = execute_tool_send(
            "jira_create",
            &serde_json::json!({
                "project": "OO",
                "issue_type": "Bug",
                "title": "Test issue"
            }),
            &memory,
            Some(&registry),
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                payload,
            } => {
                assert!(description.contains("Bug"));
                assert!(description.contains("OO"));
                assert!(description.contains("Test issue"));
                assert!(matches!(action_type, nv_core::types::ActionType::JiraCreate));
                assert_eq!(payload["project"], "OO");
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[tokio::test]
    async fn execute_jira_transition_returns_pending_action() {
        let (_dir, memory) = setup();
        let registry = make_test_registry();
        let result = execute_tool_send(
            "jira_transition",
            &serde_json::json!({
                "issue_key": "OO-42",
                "transition_name": "In Progress"
            }),
            &memory,
            Some(&registry),
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                ..
            } => {
                assert!(description.contains("OO-42"));
                assert!(description.contains("In Progress"));
                assert!(matches!(
                    action_type,
                    nv_core::types::ActionType::JiraTransition
                ));
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[tokio::test]
    async fn execute_jira_comment_returns_pending_action() {
        let (_dir, memory) = setup();
        let registry = make_test_registry();
        let result = execute_tool_send(
            "jira_comment",
            &serde_json::json!({
                "issue_key": "OO-42",
                "body": "This is a comment"
            }),
            &memory,
            Some(&registry),
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::PendingAction {
                description,
                action_type,
                ..
            } => {
                assert!(description.contains("OO-42"));
                assert!(matches!(
                    action_type,
                    nv_core::types::ActionType::JiraComment
                ));
            }
            _ => panic!("expected PendingAction"),
        }
    }

    #[test]
    fn jira_key_validation_valid() {
        assert!(validate_jira_project_key("OO").is_ok());
        assert!(validate_jira_project_key("TC").is_ok());
        assert!(validate_jira_project_key("MV").is_ok());
        assert!(validate_jira_project_key("A0").is_ok());
        assert!(validate_jira_project_key("ABCDEFGHIJ").is_ok()); // 10 chars
    }

    #[test]
    fn jira_key_validation_invalid() {
        // Too short
        assert!(validate_jira_project_key("A").is_err());
        // Empty
        assert!(validate_jira_project_key("").is_err());
        // Lowercase
        assert!(validate_jira_project_key("oo").is_err());
        assert!(validate_jira_project_key("Oo").is_err());
        // Starts with digit
        assert!(validate_jira_project_key("1A").is_err());
        // Special chars
        assert!(validate_jira_project_key("O-O").is_err());
        assert!(validate_jira_project_key("O_O").is_err());
        // Too long (11 chars)
        assert!(validate_jira_project_key("ABCDEFGHIJK").is_err());
        // Full project name (lowercase + spaces)
        assert!(validate_jira_project_key("Otaku Odyssey").is_err());
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool_send("nonexistent_tool", &serde_json::json!({}), &memory, None, &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool"));
        assert!(err.contains("nonexistent_tool"));
    }

    #[tokio::test]
    async fn execute_read_memory_missing_param() {
        let (_dir, memory) = setup();
        let result = execute_tool_send("read_memory", &serde_json::json!({}), &memory, None, &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("topic"));
    }

    #[tokio::test]
    async fn execute_write_memory_missing_content() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "write_memory",
            &serde_json::json!({"topic": "x"}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content"));
    }

    #[tokio::test]
    async fn execute_complete_bootstrap_writes_state() {
        let (_dir, memory) = setup();
        // Set HOME to a temp dir so we don't write to real ~/.nv/
        let tmp = TempDir::new().unwrap();
        let nv_dir = tmp.path().join(".nv");
        std::fs::create_dir_all(&nv_dir).unwrap();
        std::env::set_var("HOME", tmp.path());

        let result = execute_tool_send(
            "complete_bootstrap",
            &serde_json::json!({}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();

        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Bootstrap completed"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify state file was written
        let state_path = nv_dir.join("bootstrap-state.json");
        assert!(state_path.exists());
        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert!(state["completed_at"].is_string());
    }

    #[tokio::test]
    async fn execute_update_soul_writes_file() {
        let (_dir, memory) = setup();
        let tmp = TempDir::new().unwrap();
        let nv_dir = tmp.path().join(".nv");
        std::fs::create_dir_all(&nv_dir).unwrap();
        std::env::set_var("HOME", tmp.path());

        let new_soul = "# Nova — Soul\n\nUpdated personality.";
        let result = execute_tool_send(
            "update_soul",
            &serde_json::json!({"content": new_soul}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();

        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("Soul updated"));
            }
            _ => panic!("expected Immediate"),
        }

        // Verify soul.md was written
        let soul_path = nv_dir.join("soul.md");
        let content = std::fs::read_to_string(&soul_path).unwrap();
        assert_eq!(content, new_soul);
    }

    #[tokio::test]
    async fn execute_update_soul_missing_content() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "update_soul",
            &serde_json::json!({}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content"));
    }

    #[test]
    fn complete_bootstrap_schema_has_no_required_params() {
        let tools = register_tools();
        let cb = tools
            .iter()
            .find(|t| t.name == "complete_bootstrap")
            .unwrap();
        let required = cb.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn update_soul_schema_requires_content() {
        let tools = register_tools();
        let us = tools.iter().find(|t| t.name == "update_soul").unwrap();
        let required = us.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
    }

    #[test]
    fn get_recent_messages_schema_has_optional_count() {
        let tools = register_tools();
        let grm = tools
            .iter()
            .find(|t| t.name == "get_recent_messages")
            .unwrap();
        let required = grm.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
        assert!(grm.input_schema["properties"]["count"].is_object());
    }

    #[tokio::test]
    async fn execute_get_recent_messages_requires_worker_handling() {
        // execute_tool_send deliberately rejects get_recent_messages — the worker
        // must handle it directly because MessageStore (!Send) cannot cross spawn boundaries.
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "get_recent_messages",
            &serde_json::json!({}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("must be handled by the worker"),
            "expected worker-handling error"
        );
    }

    // ── ServiceRegistry<T> tests ─────────────────────────────────────

    // Minimal Checkable for testing ServiceRegistry without real network calls.
    struct StubService {
        name: String,
    }

    #[async_trait::async_trait]
    impl Checkable for StubService {
        fn name(&self) -> &str {
            &self.name
        }

        async fn check_read(&self) -> CheckResult {
            CheckResult::Healthy {
                latency_ms: 0,
                detail: "stub".to_string(),
            }
        }
    }

    fn stub(name: &str) -> StubService {
        StubService { name: name.to_string() }
    }

    #[test]
    fn service_registry_single_contains_default() {
        let reg: ServiceRegistry<StubService> = ServiceRegistry::single(stub("stripe"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        let default = reg.default().unwrap();
        assert_eq!(default.name(), "stripe");
    }

    #[test]
    fn service_registry_empty() {
        let reg: ServiceRegistry<StubService> = ServiceRegistry::new(HashMap::new(), HashMap::new());
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.default().is_none());
        assert!(reg.resolve("any").is_none());
        assert!(reg.get("any").is_none());
    }

    #[test]
    fn service_registry_get_by_name() {
        let mut instances = HashMap::new();
        instances.insert("personal".to_string(), stub("jira/personal"));
        instances.insert("llc".to_string(), stub("jira/llc"));
        let reg = ServiceRegistry::new(instances, HashMap::new());

        assert_eq!(reg.len(), 2);
        assert!(reg.get("personal").is_some());
        assert!(reg.get("llc").is_some());
        assert!(reg.get("unknown").is_none());
    }

    #[test]
    fn service_registry_resolve_via_project_map() {
        let mut instances = HashMap::new();
        instances.insert("personal".to_string(), stub("jira/personal"));
        instances.insert("llc".to_string(), stub("jira/llc"));
        let mut project_map = HashMap::new();
        project_map.insert("OO".to_string(), "personal".to_string());
        project_map.insert("CT".to_string(), "llc".to_string());
        let reg = ServiceRegistry::new(instances, project_map);

        let resolved = reg.resolve("OO").unwrap();
        assert_eq!(resolved.name(), "jira/personal");

        let resolved_ct = reg.resolve("CT").unwrap();
        assert_eq!(resolved_ct.name(), "jira/llc");
    }

    #[test]
    fn service_registry_resolve_falls_back_to_default_instance() {
        let mut instances = HashMap::new();
        instances.insert("default".to_string(), stub("stripe"));
        let reg = ServiceRegistry::new(instances, HashMap::new());

        // No project_map entry → falls back to "default"
        let resolved = reg.resolve("UNKNOWN_PROJECT").unwrap();
        assert_eq!(resolved.name(), "stripe");
    }

    #[test]
    fn service_registry_resolve_falls_back_to_first_instance() {
        let mut instances = HashMap::new();
        instances.insert("main".to_string(), stub("vercel/main"));
        let reg = ServiceRegistry::new(instances, HashMap::new());

        // No project_map, no "default" key → falls back to first
        let resolved = reg.resolve("ANY").unwrap();
        assert_eq!(resolved.name(), "vercel/main");
    }

    #[test]
    fn service_registry_iter_yields_all_instances() {
        let mut instances = HashMap::new();
        instances.insert("a".to_string(), stub("svc/a"));
        instances.insert("b".to_string(), stub("svc/b"));
        let reg = ServiceRegistry::new(instances, HashMap::new());

        let mut names: Vec<&str> = reg.iter().map(|(_, v)| v.name()).collect();
        names.sort();
        assert_eq!(names, vec!["svc/a", "svc/b"]);
    }

    #[test]
    fn service_registry_default_prefers_default_key_over_first() {
        let mut instances = HashMap::new();
        // Insert "other" before "default" to verify key-based lookup wins
        instances.insert("other".to_string(), stub("other"));
        instances.insert("default".to_string(), stub("the-default"));
        let reg = ServiceRegistry::new(instances, HashMap::new());

        let d = reg.default().unwrap();
        assert_eq!(d.name(), "the-default");
    }

    // ── spec verification tests ──────────────────────────────────────

    #[test]
    fn tool_definitions_have_no_duplicate_names() {
        let tools = register_tools();
        let total = tools.len();
        let unique: std::collections::HashSet<&str> =
            tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            unique.len(),
            total,
            "found {} duplicate tool name(s)",
            total - unique.len()
        );
    }

    #[tokio::test]
    async fn check_services_output_includes_teams_calendar_jira() {
        let (_dir, memory) = setup();
        // No env vars are set in the test environment, so all services will return
        // CheckResult::Missing — but each name still appears in the serialized JSON.
        let result = execute_tool_send(
            "check_services",
            &serde_json::json!({"read_only": true}),
            &memory,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await
        .unwrap();

        match result {
            ToolResult::Immediate(json_str) => {
                assert!(
                    json_str.contains("\"teams\""),
                    "expected 'teams' in check_services output, got: {json_str}"
                );
                assert!(
                    json_str.contains("\"calendar\""),
                    "expected 'calendar' in check_services output, got: {json_str}"
                );
                assert!(
                    json_str.contains("\"jira\""),
                    "expected 'jira' in check_services output, got: {json_str}"
                );
            }
            _ => panic!("expected Immediate result from check_services"),
        }
    }

}
