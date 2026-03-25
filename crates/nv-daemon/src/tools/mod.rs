pub use nv_tools::tools::ado;
pub use nv_tools::tools::calendar;
pub mod check;
pub mod checkable_impls;
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
use crate::aggregation;
use self::cloudflare as cloudflare_tools;
use self::doppler as doppler_tools;
use self::web as web_tools;
use crate::bash;
use self::calendar as calendar_tools;
use crate::claude::ToolDefinition;
use self::docker as docker_tools;
use self::ha as ha_tools;
use crate::memory::Memory;
use self::neon as neon_tools;
use crate::nexus;
use self::plaid as plaid_tools;
use self::posthog as posthog_tools;
use crate::reminders::{self, ReminderStore};
use self::resend as resend_tools;
use self::schedule as schedule_tools;
use self::sentry as sentry_tools;
use self::stripe as stripe_tools;
use crate::tailscale;
use self::teams as teams_tools;
use self::upstash as upstash_tools;
use self::vercel as vercel_tools;

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
            name: "query_nexus".into(),
            description: "Get the status of running Nexus agent sessions. Returns session IDs, agent names, and states.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "query_session".into(),
            description: "Get detailed information about a specific Nexus session by ID. Returns project, status, duration, command, branch, model, and cost.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to look up"
                    }
                },
                "required": ["session_id"]
            }),
        },
        ToolDefinition {
            name: "complete_bootstrap".into(),
            description: "Mark first-run bootstrap as complete. Call this after writing identity.md, user.md, and soul.md during the bootstrap conversation. Writes a state file so bootstrap is skipped on future startups.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
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

    // Add all Jira tool definitions
    tools.extend(jira::jira_tool_definitions());

    // Add scoped bash toolkit definitions
    tools.extend(bash_tool_definitions());

    // Add Docker container monitoring tools
    tools.extend(docker_tool_definitions());

    // Add Tailscale network tools
    tools.extend(tailscale_tool_definitions());

    // Add GitHub tools (gh CLI)
    tools.extend(github::github_tool_definitions());

    // Add PostHog analytics tools
    tools.extend(posthog_tool_definitions());

    // Add Vercel deployment tools
    tools.extend(vercel_tools::vercel_tool_definitions());

    // Add Sentry error tracking tools
    tools.extend(sentry_tools::sentry_tool_definitions());

    // Add Neon PostgreSQL query tools
    tools.extend(neon_tools::neon_tool_definitions());

    // Add Stripe payment data tools
    tools.extend(stripe_tools::stripe_tool_definitions());

    // Add Resend email delivery tools
    tools.extend(resend_tools::resend_tool_definitions());

    // Add Upstash Redis tools
    tools.extend(upstash_tools::upstash_tool_definitions());

    // Add Home Assistant tools
    tools.extend(ha_tools::ha_tool_definitions());

    // Add Azure DevOps tools
    tools.extend(ado_tools::ado_tool_definitions());

    // Add Plaid financial tools
    tools.extend(plaid_tools::plaid_tool_definitions());

    // Add aggregation composite tools
    tools.extend(aggregation::aggregation_tool_definitions());

    // Add Nexus project-scoped and session lifecycle tools
    tools.extend(nexus_tool_definitions());

    // Add schedule management tools
    tools.extend(schedule_tool_definitions());

    // Add Google Calendar read-only tools
    tools.extend(calendar_tool_definitions());

    // Add reminder tools (set, list, cancel)
    tools.extend(reminder_tool_definitions());

    // Add web fetch and search tools
    tools.extend(web_tools::web_tool_definitions());

    // Add Doppler secrets management tools
    tools.extend(doppler_tools::doppler_tool_definitions());

    // Add Cloudflare DNS tools
    tools.extend(cloudflare_tools::cloudflare_tool_definitions());

    // Add Microsoft Teams tools (channels, messages, send, presence)
    tools.extend(teams_tools::teams_tool_definitions());

    // Add service diagnostics tool
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

    // Add general-purpose system tools (bash, file I/O, grep)
    tools.extend(general_tool_definitions());

    // Add cross-channel routing tools
    tools.extend(vec![
        ToolDefinition {
            name: "list_channels".into(),
            description: "List available messaging channels and their connection status.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "send_to_channel".into(),
            description: "Send a message to a specific channel (telegram/discord/teams/email). Requires confirmation. For email, the recipient parameter is required.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {
                        "type": "string",
                        "description": "Target channel name (must exist in registry, e.g. 'telegram', 'discord', 'email')"
                    },
                    "message": {
                        "type": "string",
                        "description": "Message content to send"
                    },
                    "recipient": {
                        "type": "string",
                        "description": "Required for email channel — the recipient address. Ignored by other channels."
                    }
                },
                "required": ["channel", "message"]
            }),
        },
    ]);

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

/// Tool definitions for the scoped bash toolkit.
fn bash_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "git_status".into(),
            description: "Get the short git status for a project (staged, modified, untracked files).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "git_log".into(),
            description: "Get recent git commits for a project (one line per commit).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    },
                    "count": {
                        "type": "integer",
                        "description": "Number of commits to show (default: 10, max: 20)"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "git_branch".into(),
            description: "Get the current git branch name for a project.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "git_diff_stat".into(),
            description: "Get the git diff --stat summary for a project (files changed, insertions, deletions).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "ls_project".into(),
            description: "List directory contents within a project. Lists the project root if no subdir is given.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    },
                    "subdir": {
                        "type": "string",
                        "description": "Optional subdirectory within the project (e.g. 'src', 'packages/db')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "cat_config".into(),
            description: "Read a config/doc file from a project. Only .json, .toml, .yaml, .yml, .md extensions allowed.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    },
                    "file": {
                        "type": "string",
                        "description": "File path relative to project root (e.g. 'package.json', 'docs/README.md')"
                    }
                },
                "required": ["project", "file"]
            }),
        },
        ToolDefinition {
            name: "bd_ready".into(),
            description: "Get the beads ready queue for a project (issues ready for work).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
        ToolDefinition {
            name: "bd_stats".into(),
            description: "Get beads statistics for a project (issue counts, status breakdown).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
    ]
}

/// Tool definitions for Docker container monitoring.
fn docker_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "docker_status".into(),
            description: "List Docker containers with name, image, state, uptime, and ports. Returns running containers by default; pass all=true to include stopped containers.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "all": {
                        "type": "boolean",
                        "description": "Include stopped containers (default: false, only running)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "docker_logs".into(),
            description: "Get recent log lines from a Docker container. Returns the last N lines (default 50, max 200).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "container": {
                        "type": "string",
                        "description": "Container name or ID"
                    },
                    "lines": {
                        "type": "integer",
                        "description": "Number of log lines to return (default: 50, max: 200)"
                    }
                },
                "required": ["container"]
            }),
        },
    ]
}

/// Tool definitions for Tailscale network monitoring.
fn tailscale_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "tailscale_status".into(),
            description: "List all Tailscale network nodes with online/offline state, IPs, OS, and last seen time. Nodes are sorted: online first, then offline.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "tailscale_node".into(),
            description: "Get detailed info for a specific Tailscale node by hostname (case-insensitive). Returns hostname, DNSName, online, active, all IPs, OS, relay, last seen, and connection type.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The hostname of the Tailscale node to look up (case-insensitive)"
                    }
                },
                "required": ["name"]
            }),
        },
    ]
}

/// Tool definitions for PostHog analytics.
fn posthog_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "posthog_trends".into(),
            description: "Get event trend data from PostHog for a project over the last 7 days. Returns daily counts with totals and trend direction.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    },
                    "event": {
                        "type": "string",
                        "description": "PostHog event name (e.g. '$pageview', 'signup', 'purchase')"
                    }
                },
                "required": ["project", "event"]
            }),
        },
        ToolDefinition {
            name: "posthog_flags".into(),
            description: "List active feature flags from PostHog for a project. Returns flag keys, names, and rollout percentages.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl')"
                    }
                },
                "required": ["project"]
            }),
        },
    ]
}

/// Tool definitions for Nexus project-scoped queries and session lifecycle.
fn nexus_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "nexus_project_ready".into(),
            description: "Get the beads ready queue for a project via Nexus. Returns issues ready for work, scoped to the project.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_code": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl', 'nv')"
                    }
                },
                "required": ["project_code"]
            }),
        },
        ToolDefinition {
            name: "nexus_project_proposals".into(),
            description: "List open proposals in openspec/changes/ for a project. Returns proposal names and statuses (ready, proposal only, incomplete).".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_code": {
                        "type": "string",
                        "description": "Project code (e.g. 'oo', 'tc', 'tl', 'nv')"
                    }
                },
                "required": ["project_code"]
            }),
        },
        ToolDefinition {
            name: "start_session".into(),
            description: "Start a new Claude Code session on a project via Nexus. Requires confirmation before execution. Example: start_session('oo', '/apply fix-chat-bugs')".into(),
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
                        "description": "Target a specific Nexus agent by name instead of round-robin. Optional."
                    }
                },
                "required": ["project", "command"]
            }),
        },
        ToolDefinition {
            name: "send_command".into(),
            description: "Send a command to a running Nexus session. Use for remote /apply, /feature, /ci:gh execution.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to send the command to"
                    },
                    "text": {
                        "type": "string",
                        "description": "The command text to send"
                    }
                },
                "required": ["session_id", "text"]
            }),
        },
        ToolDefinition {
            name: "stop_session".into(),
            description: "Stop a running Nexus session. Requires confirmation before execution. Use to kill runaway sessions.".into(),
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
        },
        ToolDefinition {
            name: "query_nexus_health".into(),
            description: "Get machine health stats (CPU, memory, disk, load, uptime, docker containers) for all connected Nexus agents. Use to check machine load before starting sessions.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "query_nexus_projects".into(),
            description: "List available projects on all connected Nexus agents. Returns project names grouped by agent.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "query_nexus_agents".into(),
            description: "Get connection status of all configured Nexus agents. Shows which agents are connected, disconnected, or reconnecting.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
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

// ── General-purpose system tools ────────────────────────────────────

/// Production environment deny patterns.
/// Commands matching any of these are blocked before execution.
const PRODUCTION_DENY_PATTERNS: &[&str] = &[
    // Doppler production configs
    "doppler.*--config prd",
    "doppler.*--config prod",
    "doppler.*--config production",
    "DOPPLER_CONFIG=prd",
    "DOPPLER_CONFIG=prod",
    "DOPPLER_CONFIG=production",
    "doppler secrets set.*--config prd",
    // Vercel production deploys
    "vercel --prod",
    "vercel deploy --prod",
    "vercel promote",
    // Git push to main (triggers production deploys)
    "git push.*origin main",
    "git push origin main",
    // Production database writes
    "drizzle-kit push.*prod",
    "drizzle-kit migrate.*prod",
    // System-level services
    "systemctl.*restart.*--system",
    "systemctl.*stop.*--system",
    // Destructive
    "rm -rf /",
    "rm -rf ~",
    "git push --force",
];

/// Try to rewrite a command through RTK for token-optimized output.
/// Falls back to the original command if RTK is unavailable or rewrite fails.
fn try_rtk_rewrite(command: &str) -> String {
    match std::process::Command::new("rtk")
        .arg("rewrite")
        .arg(command)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => {
            let rewritten = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if rewritten.is_empty() || rewritten == command {
                command.to_string()
            } else {
                rewritten
            }
        }
        _ => command.to_string(),
    }
}

/// Check if a command matches any production deny pattern.
fn is_production_denied(command: &str) -> Option<&'static str> {
    for pattern in PRODUCTION_DENY_PATTERNS {
        // Simple substring/glob matching — patterns use .* as wildcard
        if pattern.contains(".*") {
            let parts: Vec<&str> = pattern.split(".*").collect();
            let mut pos = 0;
            let mut matched = true;
            for part in &parts {
                if let Some(found) = command[pos..].find(part) {
                    pos += found + part.len();
                } else {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Some(pattern);
            }
        } else if command.contains(pattern) {
            return Some(pattern);
        }
    }
    None
}

/// Tool definitions for general-purpose system access.
fn general_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "run_command".into(),
            description: "Execute a shell command and return stdout/stderr. \
                Commands targeting production environments are blocked. \
                Use for: package management, build tools, system diagnostics, \
                git operations (non-production), file manipulation, and any CLI tool. \
                Timeout: 30s for reads, 60s for writes.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute (e.g. 'cargo build', 'git status', 'ls -la')"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Optional working directory (absolute path). Defaults to home directory."
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "read_file".into(),
            description: "Read the contents of a file. Returns the full file content as text. \
                For large files, use the offset and limit parameters to read a specific range of lines.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional: start reading from this line number (1-based)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional: maximum number of lines to return (default: 500)"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write_file".into(),
            description: "Write content to a file (creates or overwrites). \
                Paths inside production environments or sensitive files (.env, .pem, credentials) are blocked.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to write to"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "grep_files".into(),
            description: "Search for a pattern in files under a directory. Returns matching lines with file paths and line numbers.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The search pattern (basic regex)"
                    },
                    "directory": {
                        "type": "string",
                        "description": "Absolute path to the directory to search"
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Optional: glob pattern to filter files (e.g. '*.rs', '*.ts')"
                    }
                },
                "required": ["pattern", "directory"]
            }),
        },
        ToolDefinition {
            name: "list_dir".into(),
            description: "List directory contents with file sizes and types. Works on any absolute path.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the directory"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}

/// Sensitive file patterns that write_file should block.
fn is_sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".env")
        || lower.contains(".env.")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.contains("credentials")
        || lower.contains("/secrets/")
}

/// Bootstrap-only tools — only write_memory, complete_bootstrap, and update_soul.
/// Used during first-run to prevent Claude from searching Jira/Nexus/memory
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
            name: "complete_bootstrap".into(),
            description: "Mark first-run bootstrap as complete. Call this AFTER writing identity.md, user.md, and soul.md.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
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
    let id = store.create_reminder(message, &due_at, channel)?;

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
            | "jira_create"
            | "jira_transition"
            | "jira_assign"
            | "jira_comment"
            | "start_session"
            | "stop_session"
            | "ha_service_call"
            | "send_to_channel"
            | "teams_send"
            | "complete_bootstrap"
            | "update_soul"
            | "run_command"
            | "write_file"
    )
}

/// `search_messages`, schedule tools, and reminder tools must be handled by the caller
/// before delegating here.
#[allow(clippy::too_many_arguments)]
pub async fn execute_tool_send(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
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
        "jira_search" => {
            let jql = input["jql"].as_str().ok_or_else(|| anyhow!("missing 'jql' parameter"))?;
            let registry = jira_registry.ok_or_else(|| anyhow!("Jira not configured"))?;
            // For JQL searches, use the default client (no project context in the query itself)
            let client = registry.default_client().ok_or_else(|| anyhow!("Jira not configured"))?;
            let issues = client.search(jql).await?;
            Ok(ToolResult::Immediate(jira::format_issues_for_claude(&issues)))
        }
        "jira_get" => {
            let key = input["issue_key"].as_str().ok_or_else(|| anyhow!("missing 'issue_key' parameter"))?;
            let registry = jira_registry.ok_or_else(|| anyhow!("Jira not configured"))?;
            let client = registry.resolve_from_issue_key(key)
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let issue = client.get_issue(key).await?;
            Ok(ToolResult::Immediate(jira::format_issue_for_claude(&issue)))
        }
        "jira_create" => {
            let registry = jira_registry.ok_or_else(|| anyhow!("Jira not configured"))?;
            // Validate project KEY format before queuing the pending action
            let mut project = input["project"].as_str().unwrap_or("").to_string();
            if project.is_empty() {
                if let Some(default) = registry.default_project() {
                    tracing::info!(default_project = default, "jira_create: project not provided, falling back to default");
                    project = default.to_string();
                }
            }
            validate_jira_project_key(&project)?;
            // Warn if project not found in registry (soft warning — don't block)
            if registry.resolve(&project).is_none() {
                tracing::warn!(%project, "Jira project KEY not found in registry — will attempt on approval");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraCreate,
                payload: input.clone(),
            })
        }
        "jira_transition" | "jira_assign" | "jira_comment" => {
            if jira_registry.is_none() { anyhow::bail!("Jira not configured"); }
            let description = jira::describe_pending_action(name, input);
            let action_type = match name {
                "jira_transition" => nv_core::types::ActionType::JiraTransition,
                "jira_assign" => nv_core::types::ActionType::JiraAssign,
                "jira_comment" => nv_core::types::ActionType::JiraComment,
                _ => unreachable!(),
            };
            Ok(ToolResult::PendingAction { description, action_type, payload: input.clone() })
        }
        "complete_bootstrap" => {
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home).join(".nv").join("bootstrap-state.json");
            let state = serde_json::json!({ "completed_at": chrono::Utc::now().to_rfc3339() });
            std::fs::write(&path, serde_json::to_string_pretty(&state)?).map_err(|e| anyhow!("failed to write bootstrap state: {e}"))?;
            Ok(ToolResult::Immediate("Bootstrap completed. Nova is ready.".into()))
        }
        "update_soul" => {
            let content = input["content"].as_str().ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home).join(".nv").join("soul.md");
            std::fs::write(&path, content).map_err(|e| anyhow!("failed to write soul.md: {e}"))?;
            Ok(ToolResult::Immediate("Soul updated. Notification sent to Leo.".into()))
        }
        "get_recent_messages" => Err(anyhow!("get_recent_messages must be handled by the worker directly")),
        "query_nexus" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_sessions(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_session" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let session_id = input["session_id"].as_str().ok_or_else(|| anyhow!("missing 'session_id' parameter"))?;
            let output = nexus::tools::format_query_session(client, session_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_nexus_health" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_health(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_nexus_projects" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_projects(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_nexus_agents" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_agents(client).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Nexus Project-Scoped Queries ─────────────────────────
        "nexus_project_ready" => {
            let project_code = input["project_code"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_code' parameter"))?;
            let output = nexus::tools::format_project_ready(project_code, project_registry).await?;
            Ok(ToolResult::Immediate(output))
        }
        "nexus_project_proposals" => {
            let project_code = input["project_code"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_code' parameter"))?;
            let output = nexus::tools::format_project_proposals(project_code, project_registry).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Nexus Session Lifecycle ──────────────────────────────
        "start_session" => {
            let _client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
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
        "send_command" => {
            let client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'session_id' parameter"))?;
            let text = input["text"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'text' parameter"))?;
            let output = client.send_command(session_id, text).await?;
            Ok(ToolResult::Immediate(output))
        }
        "stop_session" => {
            let _client = nexus_client.ok_or_else(|| anyhow!("Nexus not configured"))?;
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
            let output = aggregation::project_health(code, jira_client, nexus_client).await?;
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

        // ── Scoped Bash Toolkit ──────────────────────────────────
        "git_status" | "git_log" | "git_branch" | "git_diff_stat"
        | "ls_project" | "cat_config" | "bd_ready" | "bd_stats" => {
            execute_bash_tool(name, input, project_registry).await
        }

        // ── General-Purpose System Tools ────────────────────────────
        "run_command" => {
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'command' parameter"))?;
            // Enforce production deny-list
            if let Some(pattern) = is_production_denied(command) {
                anyhow::bail!("BLOCKED: command matches production deny pattern: {pattern}");
            }
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/nyaptor".into());
            let working_dir = input["working_dir"]
                .as_str()
                .unwrap_or(&home);
            let wd = std::path::Path::new(working_dir);
            if !wd.is_dir() {
                anyhow::bail!("working directory does not exist: {working_dir}");
            }
            // Try to rewrite through RTK for token-optimized output
            let effective_cmd = try_rtk_rewrite(command);
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&effective_cmd)
                .current_dir(wd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .map_err(|e| anyhow!("failed to execute command: {e}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            if output.status.success() {
                let result = if stdout.trim().is_empty() {
                    "(no output)".to_string()
                } else {
                    // Cap output at 50KB to avoid blowing context
                    let s = stdout.to_string();
                    if s.len() > 50_000 {
                        format!("{}...\n[truncated at 50KB, total {} bytes]", &s[..50_000], s.len())
                    } else {
                        s
                    }
                };
                Ok(ToolResult::Immediate(result))
            } else {
                let mut msg = format!("exit {code}");
                if !stderr.trim().is_empty() {
                    msg.push_str(&format!(": {}", stderr.trim()));
                }
                if !stdout.trim().is_empty() {
                    msg.push_str(&format!("\nstdout: {}", stdout.trim()));
                }
                Err(anyhow!("{msg}"))
            }
        }
        "read_file" => {
            let path = input["path"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'path' parameter"))?;
            if path.contains("..") {
                anyhow::bail!("path traversal not allowed");
            }
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| anyhow!("failed to read {path}: {e}"))?;
            let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
            let limit = input["limit"].as_u64().unwrap_or(500) as usize;
            let lines: Vec<&str> = content.lines().collect();
            let start = (offset - 1).min(lines.len());
            let end = (start + limit).min(lines.len());
            let slice: Vec<String> = lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{:>5} {}", start + i + 1, l))
                .collect();
            if slice.is_empty() {
                Ok(ToolResult::Immediate("(empty file)".into()))
            } else {
                Ok(ToolResult::Immediate(slice.join("\n")))
            }
        }
        "write_file" => {
            let path = input["path"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'path' parameter"))?;
            let content = input["content"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            if path.contains("..") {
                anyhow::bail!("path traversal not allowed");
            }
            if is_sensitive_path(path) {
                anyhow::bail!("BLOCKED: cannot write to sensitive file: {path}");
            }
            // Ensure parent directory exists
            if let Some(parent) = std::path::Path::new(path).parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| anyhow!("failed to create parent directory: {e}"))?;
            }
            tokio::fs::write(path, content)
                .await
                .map_err(|e| anyhow!("failed to write {path}: {e}"))?;
            Ok(ToolResult::Immediate(format!("Written {} bytes to {path}", content.len())))
        }
        "grep_files" => {
            let pattern = input["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'pattern' parameter"))?;
            let directory = input["directory"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'directory' parameter"))?;
            if directory.contains("..") {
                anyhow::bail!("path traversal not allowed");
            }
            let mut cmd_str = format!("grep -rn --include='*' '{}' '{}'", pattern, directory);
            if let Some(fp) = input["file_pattern"].as_str() {
                cmd_str = format!("grep -rn --include='{}' '{}' '{}'", fp, pattern, directory);
            }
            // Route through RTK for token-optimized output
            let effective_cmd = try_rtk_rewrite(&cmd_str);
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&effective_cmd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .map_err(|e| anyhow!("grep failed: {e}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                Ok(ToolResult::Immediate("No matches found.".into()))
            } else {
                let s = stdout.to_string();
                let result = if s.len() > 50_000 {
                    format!("{}...\n[truncated at 50KB]", &s[..50_000])
                } else {
                    s
                };
                Ok(ToolResult::Immediate(result))
            }
        }
        "list_dir" => {
            let path = input["path"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'path' parameter"))?;
            if path.contains("..") {
                anyhow::bail!("path traversal not allowed");
            }
            // Route through RTK for token-optimized output
            let cmd_str = format!("ls -la '{}'", path);
            let effective_cmd = try_rtk_rewrite(&cmd_str);
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&effective_cmd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .map_err(|e| anyhow!("ls failed: {e}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                Ok(ToolResult::Immediate("(empty directory)".into()))
            } else {
                Ok(ToolResult::Immediate(stdout.to_string()))
            }
        }

        // ── Docker Tools ────────────────────────────────────────────
        "docker_status" => {
            let all = input["all"].as_bool().unwrap_or(false);
            let output = docker_tools::docker_status(all).await?;
            Ok(ToolResult::Immediate(output))
        }
        "docker_logs" => {
            let container = input["container"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'container' parameter"))?;
            let lines = input["lines"].as_u64();
            let output = docker_tools::docker_logs(container, lines).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── GitHub Tools ─────────────────────────────────────────────
        "gh_pr_list" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_pr_list(repo).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_run_status" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_run_status(repo).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_issues" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_issues(repo).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_pr_detail" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let pr_number = input["pr_number"].as_u64().ok_or_else(|| anyhow!("missing or invalid 'pr_number' parameter"))?;
            let output = github::gh_pr_detail(repo, pr_number).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_pr_diff" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let pr_number = input["pr_number"].as_u64().ok_or_else(|| anyhow!("missing or invalid 'pr_number' parameter"))?;
            let file_filter = input["file_filter"].as_str();
            let output = github::gh_pr_diff(repo, pr_number, file_filter).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_releases" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let limit = input["limit"].as_u64();
            let output = github::gh_releases(repo, limit).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_compare" => {
            let repo = input["repo"].as_str().ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let base = input["base"].as_str().ok_or_else(|| anyhow!("missing 'base' parameter"))?;
            let head = input["head"].as_str().ok_or_else(|| anyhow!("missing 'head' parameter"))?;
            let output = github::gh_compare(repo, base, head).await?;
            Ok(ToolResult::Immediate(output))
        }


        // ── Tailscale Tools ──────────────────────────────────────────
        "tailscale_status" => {
            let output = tailscale::TailscaleClient::status().await?;
            Ok(ToolResult::Immediate(output))
        }
        "tailscale_node" => {
            let name_param = input["name"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'name' parameter"))?;
            let output = tailscale::TailscaleClient::node(name_param).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── PostHog Tools ───────────────────────────────────────────
        "posthog_trends" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let event = input["event"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'event' parameter"))?;
            let output = posthog_tools::query_trends(project, event).await?;
            Ok(ToolResult::Immediate(output))
        }
        "posthog_flags" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let output = posthog_tools::list_flags(project).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Vercel Tools ─────────────────────────────────────────────
        "vercel_deployments" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let _owned_vercel;
            let client = if let Some(reg) = service_registries.vercel {
                reg.resolve(project)
                    .or_else(|| reg.default())
                    .ok_or_else(|| anyhow!("Vercel registry empty"))?
            } else {
                _owned_vercel = vercel_tools::VercelClient::from_env()?;
                &_owned_vercel
            };
            let output = vercel_tools::vercel_deployments(client, project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "vercel_logs" => {
            let deploy_id = input["deploy_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'deploy_id' parameter"))?;
            let _owned_vercel;
            let client = if let Some(reg) = service_registries.vercel {
                reg.default().ok_or_else(|| anyhow!("Vercel registry empty"))?
            } else {
                _owned_vercel = vercel_tools::VercelClient::from_env()?;
                &_owned_vercel
            };
            let output = vercel_tools::vercel_logs(client, deploy_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Sentry Tools ────────────────────────────────────────────
        "sentry_issues" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let _owned_sentry;
            let client = if let Some(reg) = service_registries.sentry {
                reg.resolve(project)
                    .or_else(|| reg.default())
                    .ok_or_else(|| anyhow!("Sentry registry empty"))?
            } else {
                _owned_sentry = sentry_tools::SentryClient::from_env()?;
                &_owned_sentry
            };
            let output = sentry_tools::sentry_issues(client, project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "sentry_issue" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            let _owned_sentry;
            let client = if let Some(reg) = service_registries.sentry {
                reg.default().ok_or_else(|| anyhow!("Sentry registry empty"))?
            } else {
                _owned_sentry = sentry_tools::SentryClient::from_env()?;
                &_owned_sentry
            };
            let output = sentry_tools::sentry_issue(client, id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Neon Tools ──────────────────────────────────────────────
        "neon_query" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let sql = input["sql"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'sql' parameter"))?;
            let output = neon_tools::neon_query(project, sql).await?;
            Ok(ToolResult::Immediate(output))
        }
        "neon_projects" => {
            let output = neon_tools::neon_projects().await?;
            Ok(ToolResult::Immediate(output))
        }
        "neon_branches" => {
            let project_id = input["project_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_id' parameter"))?;
            let output = neon_tools::neon_branches(project_id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "neon_compute" => {
            let project_id = input["project_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project_id' parameter"))?;
            let branch_id = input["branch_id"].as_str();
            let output = neon_tools::neon_compute(project_id, branch_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Stripe Tools ────────────────────────────────────────────
        "stripe_customers" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            let _owned_stripe;
            let client = if let Some(reg) = service_registries.stripe {
                reg.default().ok_or_else(|| anyhow!("Stripe registry empty"))?
            } else {
                _owned_stripe = stripe_tools::StripeClient::from_env()?;
                &_owned_stripe
            };
            let output = stripe_tools::stripe_customers(client, query).await?;
            Ok(ToolResult::Immediate(output))
        }
        "stripe_invoices" => {
            let status = input["status"]
                .as_str()
                .unwrap_or("open");
            let _owned_stripe;
            let client = if let Some(reg) = service_registries.stripe {
                reg.default().ok_or_else(|| anyhow!("Stripe registry empty"))?
            } else {
                _owned_stripe = stripe_tools::StripeClient::from_env()?;
                &_owned_stripe
            };
            let output = stripe_tools::stripe_invoices(client, status).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Resend Tools ────────────────────────────────────────────
        "resend_emails" => {
            let status = input["status"].as_str();
            let output = resend_tools::resend_emails(status).await?;
            Ok(ToolResult::Immediate(output))
        }
        "resend_bounces" => {
            let output = resend_tools::resend_bounces().await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Upstash Tools ───────────────────────────────────────────
        "upstash_info" => {
            let output = upstash_tools::upstash_info().await?;
            Ok(ToolResult::Immediate(output))
        }
        "upstash_keys" => {
            let pattern = input["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'pattern' parameter"))?;
            let output = upstash_tools::upstash_keys(pattern).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Home Assistant Tools ─────────────────────────────────────
        "ha_states" => {
            let output = ha_tools::ha_states().await?;
            Ok(ToolResult::Immediate(output))
        }
        "ha_entity" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            let output = ha_tools::ha_entity(id).await?;
            Ok(ToolResult::Immediate(output))
        }
        "ha_service_call" => {
            let domain = input["domain"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'domain' parameter"))?;
            let service = input["service"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'service' parameter"))?;
            let data = input
                .get("data")
                .ok_or_else(|| anyhow!("missing 'data' parameter"))?;
            let description = ha_tools::describe_service_call(domain, service, data);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::HaServiceCall,
                payload: input.clone(),
            })
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

        // ── Plaid Financial Tools ────────────────────────────────────
        "plaid_balances" => {
            let output = plaid_tools::plaid_balances().await?;
            Ok(ToolResult::Immediate(output))
        }
        "plaid_bills" => {
            let output = plaid_tools::plaid_bills().await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Cross-Channel Routing ─────────────────────────────────
        "list_channels" => {
            let lines = if channels.is_empty() {
                vec!["No channels configured.".to_string()]
            } else {
                let mut l = vec!["Available channels:".to_string()];
                let mut names: Vec<&String> = channels.keys().collect();
                names.sort();
                for ch_name in names {
                    l.push(format!("- {ch_name} (connected)"));
                }
                l
            };
            Ok(ToolResult::Immediate(lines.join("\n")))
        }
        "send_to_channel" => {
            let channel_name = input["channel"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'channel' parameter"))?;
            let message = input["message"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'message' parameter"))?;

            // Validate channel exists
            if !channels.contains_key(channel_name) {
                let available: Vec<&str> = channels.keys().map(|s| s.as_str()).collect();
                anyhow::bail!(
                    "Channel '{}' not found. Available channels: {}",
                    channel_name,
                    available.join(", ")
                );
            }

            // Validate message non-empty
            if message.trim().is_empty() {
                anyhow::bail!("message must be non-empty");
            }

            // Validate recipient required for email
            if channel_name == "email" && input["recipient"].as_str().is_none() {
                anyhow::bail!("recipient is required for the email channel");
            }

            // Build human-readable description for confirmation keyboard
            let preview = if message.len() > 60 {
                format!("{}…", &message[..60])
            } else {
                message.to_string()
            };
            let description = format!("Send to {channel_name}: \"{preview}\"");

            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::ChannelSend,
                payload: input.clone(),
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

        // ── Web Fetch Tools ────────────────────────────────────────────
        "fetch_url" => {
            let url = input["url"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'url' parameter"))?;
            let format_hint = input["format"].as_str();
            let output = web_tools::fetch_url(url, format_hint).await?;
            Ok(ToolResult::Immediate(output))
        }
        "check_url" => {
            let url = input["url"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'url' parameter"))?;
            let output = web_tools::check_url(url).await?;
            Ok(ToolResult::Immediate(output))
        }
        "search_web" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            let count = input["count"].as_u64().unwrap_or(5) as usize;
            // search_url: use NV_WEB_SEARCH_URL env var if set, otherwise None (defaults to DDG)
            let search_url_env = std::env::var("NV_WEB_SEARCH_URL").ok();
            let search_url = search_url_env.as_deref();
            let output = web_tools::search_web(query, count, search_url).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Doppler Tools ──────────────────────────────────────────────
        "doppler_secrets" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let environment = input["environment"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'environment' parameter"))?;
            let _owned_doppler;
            let client = if let Some(reg) = service_registries.doppler {
                reg.default().ok_or_else(|| anyhow!("Doppler registry empty"))?
            } else {
                _owned_doppler = doppler_tools::DopplerClient::from_env()?;
                &_owned_doppler
            };
            let output = doppler_tools::doppler_secrets(client, project, environment, None).await?;
            Ok(ToolResult::Immediate(output))
        }
        "doppler_compare" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let env_a = input["env_a"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'env_a' parameter"))?;
            let env_b = input["env_b"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'env_b' parameter"))?;
            let _owned_doppler;
            let client = if let Some(reg) = service_registries.doppler {
                reg.default().ok_or_else(|| anyhow!("Doppler registry empty"))?
            } else {
                _owned_doppler = doppler_tools::DopplerClient::from_env()?;
                &_owned_doppler
            };
            let output = doppler_tools::doppler_compare(client, project, env_a, env_b, None).await?;
            Ok(ToolResult::Immediate(output))
        }
        "doppler_activity" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let count = input["count"].as_u64();
            let _owned_doppler;
            let client = if let Some(reg) = service_registries.doppler {
                reg.default().ok_or_else(|| anyhow!("Doppler registry empty"))?
            } else {
                _owned_doppler = doppler_tools::DopplerClient::from_env()?;
                &_owned_doppler
            };
            let output = doppler_tools::doppler_activity(client, project, count, None).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Cloudflare DNS Tools ───────────────────────────────────────
        "cf_zones" => {
            let _owned_cf;
            let client = if let Some(reg) = service_registries.cloudflare {
                reg.default().ok_or_else(|| anyhow!("Cloudflare registry empty"))?
            } else {
                _owned_cf = cloudflare_tools::CloudflareClient::from_env()?;
                &_owned_cf
            };
            let output = cloudflare_tools::cf_zones(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "cf_dns_records" => {
            let domain = input["domain"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'domain' parameter"))?;
            let record_type = input["record_type"].as_str();
            let _owned_cf;
            let client = if let Some(reg) = service_registries.cloudflare {
                reg.default().ok_or_else(|| anyhow!("Cloudflare registry empty"))?
            } else {
                _owned_cf = cloudflare_tools::CloudflareClient::from_env()?;
                &_owned_cf
            };
            let output = cloudflare_tools::cf_dns_records(client, domain, record_type).await?;
            Ok(ToolResult::Immediate(output))
        }
        "cf_domain_status" => {
            let domain = input["domain"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'domain' parameter"))?;
            let _owned_cf;
            let client = if let Some(reg) = service_registries.cloudflare {
                reg.default().ok_or_else(|| anyhow!("Cloudflare registry empty"))?
            } else {
                _owned_cf = cloudflare_tools::CloudflareClient::from_env()?;
                &_owned_cf
            };
            let output = cloudflare_tools::cf_domain_status(client, domain).await?;
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



/// Execute a scoped bash tool by parsing input and delegating to `bash::execute_command`.
async fn execute_bash_tool(
    name: &str,
    input: &serde_json::Value,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<ToolResult> {
    let project = input["project"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
    let project_root = bash::validate_project(project, project_registry)?;

    let cmd = match name {
        "git_status" => bash::AllowedCommand::GitStatus,
        "git_log" => {
            let count = input["count"].as_u64().unwrap_or(10);
            bash::AllowedCommand::GitLog { count }
        }
        "git_branch" => bash::AllowedCommand::GitBranch,
        "git_diff_stat" => bash::AllowedCommand::GitDiffStat,
        "ls_project" => {
            let subdir = input["subdir"].as_str().map(String::from);
            bash::AllowedCommand::LsDir { subdir }
        }
        "cat_config" => {
            let file = input["file"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'file' parameter"))?;
            bash::AllowedCommand::CatConfig {
                file: file.to_string(),
            }
        }
        "bd_ready" => bash::AllowedCommand::BdReady,
        "bd_stats" => bash::AllowedCommand::BdStats,
        _ => unreachable!(),
    };

    let output = bash::execute_command(&cmd, project_root).await?;
    Ok(ToolResult::Immediate(output))
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
        // 3 memory + 2 messages (get_recent + search) + 2 bootstrap/soul + 5 nexus (query + session + health + projects + agents)
        // + 6 jira + 8 bash
        // + 2 docker + 2 tailscale + 3 github + 2 sentry + 2 posthog + 2 vercel
        // + 4 neon (neon_query + neon_projects + neon_branches + neon_compute) + 2 stripe + 2 resend + 2 upstash
        // + 3 ha + 3 ado + 2 plaid + 3 aggregation
        // + 5 nexus lifecycle (project_ready, project_proposals, start_session, send_command, stop_session)
        // + 2 cross-channel (list_channels, send_to_channel)
        // + 3 calendar (calendar_today, calendar_upcoming, calendar_next)
        // + 3 reminders (set_reminder, list_reminders, cancel_reminder)
        // + 3 web (fetch_url, check_url, search_web)
        // + 3 doppler (doppler_secrets, doppler_compare, doppler_activity)
        // + 3 cloudflare (cf_zones, cf_dns_records, cf_domain_status)
        // + 3 schedule (list_schedules, add_schedule, remove_schedule)
        // + 1 check_services
        // + 4 teams (teams_channels, teams_messages, teams_send, teams_presence)
        // = 98 - 3 (removed duplicate query_nexus_health/projects/agents) = 95
        assert_eq!(tools.len(), 95);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_memory"));
        assert!(names.contains(&"search_memory"));
        assert!(names.contains(&"write_memory"));
        assert!(names.contains(&"get_recent_messages"));
        assert!(names.contains(&"search_messages"));
        assert!(names.contains(&"complete_bootstrap"));
        assert!(names.contains(&"update_soul"));
        assert!(names.contains(&"query_nexus"));
        assert!(names.contains(&"query_session"));
        assert!(names.contains(&"query_nexus_health"));
        assert!(names.contains(&"query_nexus_projects"));
        assert!(names.contains(&"query_nexus_agents"));
        assert!(names.contains(&"jira_search"));
        assert!(names.contains(&"jira_get"));
        assert!(names.contains(&"jira_create"));
        assert!(names.contains(&"jira_transition"));
        assert!(names.contains(&"jira_assign"));
        assert!(names.contains(&"jira_comment"));
        // Bash toolkit tools
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_log"));
        assert!(names.contains(&"git_branch"));
        assert!(names.contains(&"git_diff_stat"));
        assert!(names.contains(&"ls_project"));
        assert!(names.contains(&"cat_config"));
        assert!(names.contains(&"bd_ready"));
        assert!(names.contains(&"bd_stats"));
        // Docker tools
        assert!(names.contains(&"docker_status"));
        assert!(names.contains(&"docker_logs"));
        // Tailscale tools
        assert!(names.contains(&"tailscale_status"));
        assert!(names.contains(&"tailscale_node"));
        // GitHub tools
        assert!(names.contains(&"gh_pr_list"));
        assert!(names.contains(&"gh_run_status"));
        assert!(names.contains(&"gh_issues"));
        assert!(names.contains(&"gh_pr_detail"));
        assert!(names.contains(&"gh_pr_diff"));
        assert!(names.contains(&"gh_releases"));
        assert!(names.contains(&"gh_compare"));
        // Sentry tools
        assert!(names.contains(&"sentry_issues"));
        assert!(names.contains(&"sentry_issue"));
        // PostHog tools
        assert!(names.contains(&"posthog_trends"));
        assert!(names.contains(&"posthog_flags"));
        // Vercel tools
        assert!(names.contains(&"vercel_deployments"));
        assert!(names.contains(&"vercel_logs"));
        // Neon tools
        assert!(names.contains(&"neon_query"));
        assert!(names.contains(&"neon_projects"));
        assert!(names.contains(&"neon_branches"));
        assert!(names.contains(&"neon_compute"));
        // Home Assistant tools
        assert!(names.contains(&"ha_states"));
        assert!(names.contains(&"ha_entity"));
        assert!(names.contains(&"ha_service_call"));
        // Azure DevOps tools
        assert!(names.contains(&"ado_projects"));
        assert!(names.contains(&"ado_pipelines"));
        assert!(names.contains(&"ado_builds"));
        // Plaid tools
        assert!(names.contains(&"plaid_balances"));
        assert!(names.contains(&"plaid_bills"));
        // Aggregation tools
        assert!(names.contains(&"project_health"));
        assert!(names.contains(&"homelab_status"));
        assert!(names.contains(&"financial_summary"));
        // Nexus lifecycle tools
        assert!(names.contains(&"nexus_project_ready"));
        assert!(names.contains(&"nexus_project_proposals"));
        assert!(names.contains(&"start_session"));
        assert!(names.contains(&"send_command"));
        assert!(names.contains(&"stop_session"));
        // Cross-channel routing tools
        assert!(names.contains(&"list_channels"));
        assert!(names.contains(&"send_to_channel"));
        // Service diagnostics
        assert!(names.contains(&"check_services"));
        // Teams tools
        assert!(names.contains(&"teams_channels"));
        assert!(names.contains(&"teams_messages"));
        assert!(names.contains(&"teams_send"));
        assert!(names.contains(&"teams_presence"));
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

    #[test]
    fn query_nexus_schema_has_no_required_params() {
        let tools = register_tools();
        let qn = tools.iter().find(|t| t.name == "query_nexus").unwrap();
        let required = qn.input_schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[tokio::test]
    async fn execute_read_memory_returns_content() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "read_memory",
            &serde_json::json!({"topic": "tasks"}),
            &memory,
            None,
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
    async fn execute_query_nexus_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool_send("query_nexus", &serde_json::json!({}), &memory, None, None, &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn execute_query_nexus_with_client_returns_immediate() {
        let (_dir, memory) = setup();
        let client = nexus::client::NexusClient::new(&[nv_core::config::NexusAgent {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port: 7400,
        }]);
        let result = execute_tool_send("query_nexus", &serde_json::json!({}), &memory, None, Some(&client), &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries())
            .await
            .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("unreachable") || s.contains("No Nexus agents") || s.contains("No active sessions"));
            }
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_query_session_without_client_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool_send(
            "query_session",
            &serde_json::json!({"session_id": "s-1"}),
            &memory,
            None,
            None,
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn execute_query_session_missing_param() {
        let (_dir, memory) = setup();
        let client = nexus::client::NexusClient::new(&[]);
        let result = execute_tool_send(
            "query_session",
            &serde_json::json!({}),
            &memory,
            None,
            Some(&client),
            &empty_registry(),
            &empty_channels(),
            None,
            "primary",
            &empty_service_registries(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("session_id"));
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool_send("nonexistent_tool", &serde_json::json!({}), &memory, None, None, &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool"));
        assert!(err.contains("nonexistent_tool"));
    }

    #[tokio::test]
    async fn execute_read_memory_missing_param() {
        let (_dir, memory) = setup();
        let result = execute_tool_send("read_memory", &serde_json::json!({}), &memory, None, None, &empty_registry(), &empty_channels(), None, "primary", &empty_service_registries()).await;
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
