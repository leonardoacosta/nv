use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::ado_tools;
use crate::aggregation;
use crate::bash;
use crate::claude::ToolDefinition;
use crate::docker_tools;
use crate::github;
use crate::ha_tools;
use crate::jira;
use crate::memory::Memory;
use crate::messages::MessageStore;
use crate::neon_tools;
use crate::nexus;
use crate::plaid_tools;
use crate::posthog_tools;
use crate::resend_tools;
use crate::sentry_tools;
use crate::stripe_tools;
use crate::tailscale;
use crate::upstash_tools;
use crate::vercel_tools;

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

    tools
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
    ]
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

/// Execute a tool without access to `MessageStore` (Send-safe).
///
/// This variant avoids referencing `MessageStore` (which wraps
/// `rusqlite::Connection`, a `!Send` type) so the resulting future
/// can be used with `tokio::spawn`. The `get_recent_messages` tool
/// must be handled by the caller before delegating to this function.
pub async fn execute_tool_send(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<ToolResult> {
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
            let project = input["project"].as_str().unwrap_or("");
            validate_jira_project_key(project)?;
            // Warn if project not found in registry (soft warning — don't block)
            if registry.resolve(project).is_none() {
                tracing::warn!(project, "Jira project KEY not found in registry — will attempt on approval");
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
            let description = format!(
                "Start CC session on {}: `{}`",
                project.to_uppercase(),
                command
            );
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
            let client = vercel_tools::VercelClient::from_env()?;
            let output = vercel_tools::vercel_deployments(&client, project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "vercel_logs" => {
            let deploy_id = input["deploy_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'deploy_id' parameter"))?;
            let client = vercel_tools::VercelClient::from_env()?;
            let output = vercel_tools::vercel_logs(&client, deploy_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Sentry Tools ────────────────────────────────────────────
        "sentry_issues" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let output = sentry_tools::sentry_issues(project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "sentry_issue" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            let output = sentry_tools::sentry_issue(id).await?;
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

        // ── Stripe Tools ────────────────────────────────────────────
        "stripe_customers" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            let output = stripe_tools::stripe_customers(query).await?;
            Ok(ToolResult::Immediate(output))
        }
        "stripe_invoices" => {
            let status = input["status"]
                .as_str()
                .unwrap_or("open");
            let output = stripe_tools::stripe_invoices(status).await?;
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

        _ => Err(anyhow!("unknown tool: {name}")),
    }
}

/// Execute a tool by name with the given input parameters.
///
/// Memory tools are synchronous. Jira read tools are async. Jira write
/// tools return a PendingAction instead of executing immediately.
///
/// NOTE: This function takes `Option<&MessageStore>`, making the resulting
/// future `!Send`. For use in `tokio::spawn`, use `execute_tool_send` instead.
#[allow(dead_code)]
pub async fn execute_tool(
    name: &str,
    input: &serde_json::Value,
    memory: &Memory,
    jira_registry: Option<&jira::JiraRegistry>,
    nexus_client: Option<&nexus::client::NexusClient>,
    message_store: Option<&MessageStore>,
    project_registry: &HashMap<String, PathBuf>,
) -> Result<ToolResult> {
    match name {
        // ── Memory Tools ────────────────────────────────────────
        "read_memory" => {
            let topic = input["topic"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            memory.read(topic).map(ToolResult::Immediate)
        }
        "search_memory" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            memory.search(query).map(ToolResult::Immediate)
        }
        "write_memory" => {
            let topic = input["topic"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'topic' parameter"))?;
            let content = input["content"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            memory.write(topic, content).map(ToolResult::Immediate)
        }

        // ── Jira Read Tools (immediate) ─────────────────────────
        "jira_search" => {
            let jql = input["jql"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'jql' parameter"))?;
            let registry = jira_registry
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let client = registry.default_client()
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let issues = client.search(jql).await?;
            Ok(ToolResult::Immediate(jira::format_issues_for_claude(
                &issues,
            )))
        }
        "jira_get" => {
            let key = input["issue_key"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'issue_key' parameter"))?;
            let registry = jira_registry
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let client = registry.resolve_from_issue_key(key)
                .ok_or_else(|| anyhow!("Jira not configured"))?;
            let issue = client.get_issue(key).await?;
            Ok(ToolResult::Immediate(jira::format_issue_for_claude(&issue)))
        }

        // ── Jira Write Tools (pending action) ──────────────────
        "jira_create" => {
            let registry = jira_registry.ok_or_else(|| anyhow!("Jira not configured"))?;
            let project = input["project"].as_str().unwrap_or("");
            validate_jira_project_key(project)?;
            if registry.resolve(project).is_none() {
                tracing::warn!(project, "Jira project KEY not found in registry");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraCreate,
                payload: input.clone(),
            })
        }
        "jira_transition" => {
            if jira_registry.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraTransition,
                payload: input.clone(),
            })
        }
        "jira_assign" => {
            if jira_registry.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraAssign,
                payload: input.clone(),
            })
        }
        "jira_comment" => {
            if jira_registry.is_none() {
                anyhow::bail!("Jira not configured");
            }
            let description = jira::describe_pending_action(name, input);
            Ok(ToolResult::PendingAction {
                description,
                action_type: nv_core::types::ActionType::JiraComment,
                payload: input.clone(),
            })
        }

        // ── Bootstrap & Soul Tools ──────────────────────────
        "complete_bootstrap" => {
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home)
                .join(".nv")
                .join("bootstrap-state.json");
            let state = serde_json::json!({
                "completed_at": chrono::Utc::now().to_rfc3339()
            });
            std::fs::write(&path, serde_json::to_string_pretty(&state)?)
                .map_err(|e| anyhow!("failed to write bootstrap state: {e}"))?;
            tracing::info!("bootstrap completed, state written");
            Ok(ToolResult::Immediate(
                "Bootstrap completed. Nova is ready.".into(),
            ))
        }
        "update_soul" => {
            let content = input["content"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'content' parameter"))?;
            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::Path::new(&home).join(".nv").join("soul.md");
            std::fs::write(&path, content)
                .map_err(|e| anyhow!("failed to write soul.md: {e}"))?;
            tracing::info!("soul.md updated");
            Ok(ToolResult::Immediate(
                "Soul updated. Notification sent to Leo.".into(),
            ))
        }

        // ── Message Store Tools ─────────────────────────────
        "get_recent_messages" => {
            let store = message_store
                .ok_or_else(|| anyhow!("Message store not available"))?;
            let count = input["count"]
                .as_u64()
                .unwrap_or(20)
                .min(100) as usize;
            let messages = store.recent(count)?;
            if messages.is_empty() {
                return Ok(ToolResult::Immediate("No messages in history.".into()));
            }
            let mut lines = Vec::with_capacity(messages.len());
            for msg in &messages {
                let time_part = if msg.timestamp.len() >= 16 {
                    &msg.timestamp[11..16]
                } else {
                    &msg.timestamp
                };
                let sender = if msg.direction == "outbound" {
                    "Nova"
                } else {
                    &msg.sender
                };
                lines.push(format!("[{time_part}] {sender}: {}", msg.content));
            }
            Ok(ToolResult::Immediate(lines.join("\n")))
        }

        // ── Nexus Tools ──────────────────────────────────────
        "query_nexus" => {
            let client = nexus_client
                .ok_or_else(|| anyhow!("Nexus not configured"))?;
            let output = nexus::tools::format_query_sessions(client).await?;
            Ok(ToolResult::Immediate(output))
        }
        "query_session" => {
            let client = nexus_client
                .ok_or_else(|| anyhow!("Nexus not configured"))?;
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'session_id' parameter"))?;
            let output = nexus::tools::format_query_session(client, session_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Aggregation Tools ────────────────────────────────────
        "project_health" => {
            let code = input["code"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'code' parameter"))?;
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
            let repo = input["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_pr_list(repo).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_run_status" => {
            let repo = input["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_run_status(repo).await?;
            Ok(ToolResult::Immediate(output))
        }
        "gh_issues" => {
            let repo = input["repo"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'repo' parameter"))?;
            let output = github::gh_issues(repo).await?;
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
            let client = vercel_tools::VercelClient::from_env()?;
            let output = vercel_tools::vercel_deployments(&client, project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "vercel_logs" => {
            let deploy_id = input["deploy_id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'deploy_id' parameter"))?;
            let client = vercel_tools::VercelClient::from_env()?;
            let output = vercel_tools::vercel_logs(&client, deploy_id).await?;
            Ok(ToolResult::Immediate(output))
        }

        // ── Sentry Tools ────────────────────────────────────────────
        "sentry_issues" => {
            let project = input["project"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'project' parameter"))?;
            let output = sentry_tools::sentry_issues(project).await?;
            Ok(ToolResult::Immediate(output))
        }
        "sentry_issue" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'id' parameter"))?;
            let output = sentry_tools::sentry_issue(id).await?;
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

        // ── Stripe Tools ────────────────────────────────────────────
        "stripe_customers" => {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| anyhow!("missing 'query' parameter"))?;
            let output = stripe_tools::stripe_customers(query).await?;
            Ok(ToolResult::Immediate(output))
        }
        "stripe_invoices" => {
            let status = input["status"]
                .as_str()
                .unwrap_or("open");
            let output = stripe_tools::stripe_invoices(status).await?;
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

        _ => Err(anyhow!("unknown tool: {name}")),
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

    #[test]
    fn register_tools_returns_expected_count() {
        let tools = register_tools();
        // 3 memory + 2 messages (get_recent + search) + 2 bootstrap/soul + 2 nexus + 6 jira + 8 bash
        // + 2 docker + 2 tailscale + 3 github + 2 sentry + 2 posthog + 2 vercel
        // + 1 neon + 2 stripe + 2 resend + 2 upstash
        // + 3 ha + 2 ado + 2 plaid + 3 aggregation
        // + 5 nexus lifecycle (project_ready, project_proposals, start_session, send_command, stop_session) = 58
        assert_eq!(tools.len(), 58);

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
        // Sentry tools
        assert!(names.contains(&"sentry_issues"));
        assert!(names.contains(&"sentry_issue"));
        // PostHog tools
        assert!(names.contains(&"posthog_trends"));
        assert!(names.contains(&"posthog_flags"));
        // Vercel tools
        assert!(names.contains(&"vercel_deployments"));
        assert!(names.contains(&"vercel_logs"));
        // Home Assistant tools
        assert!(names.contains(&"ha_states"));
        assert!(names.contains(&"ha_entity"));
        assert!(names.contains(&"ha_service_call"));
        // Azure DevOps tools
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
        let result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "tasks"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "nonexistent"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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

        let result = execute_tool(
            "search_memory",
            &serde_json::json!({"query": "Stripe"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "search_memory",
            &serde_json::json!({"query": "xyznonexistent"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "write_memory",
            &serde_json::json!({"topic": "notes", "content": "hello world"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let read_result = execute_tool(
            "read_memory",
            &serde_json::json!({"topic": "notes"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "jira_search",
            &serde_json::json!({"jql": "project = NV"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
            jira_api_token: Some("fake-token".to_string()),
            jira_username: Some("test@test.com".to_string()),
            elevenlabs_api_key: None,
            jira_api_tokens: HashMap::new(),
            jira_usernames: HashMap::new(),
        };
        jira::JiraRegistry::new(&cfg, &secrets).unwrap().unwrap()
    }

    #[tokio::test]
    async fn execute_jira_create_returns_pending_action() {
        let (_dir, memory) = setup();
        let registry = make_test_registry();
        let result = execute_tool(
            "jira_create",
            &serde_json::json!({
                "project": "OO",
                "issue_type": "Bug",
                "title": "Test issue"
            }),
            &memory,
            Some(&registry),
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "jira_transition",
            &serde_json::json!({
                "issue_key": "OO-42",
                "transition_name": "In Progress"
            }),
            &memory,
            Some(&registry),
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "jira_comment",
            &serde_json::json!({
                "issue_key": "OO-42",
                "body": "This is a comment"
            }),
            &memory,
            Some(&registry),
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool("query_nexus", &serde_json::json!({}), &memory, None, None, None, &empty_registry())
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
        let result = execute_tool("query_nexus", &serde_json::json!({}), &memory, None, Some(&client), None, &empty_registry())
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
        let result = execute_tool(
            "query_session",
            &serde_json::json!({"session_id": "s-1"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Nexus not configured"));
    }

    #[tokio::test]
    async fn execute_query_session_missing_param() {
        let (_dir, memory) = setup();
        let client = nexus::client::NexusClient::new(&[]);
        let result = execute_tool(
            "query_session",
            &serde_json::json!({}),
            &memory,
            None,
            Some(&client),
            None,
            &empty_registry(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("session_id"));
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error() {
        let (_dir, memory) = setup();
        let result = execute_tool("nonexistent_tool", &serde_json::json!({}), &memory, None, None, None, &empty_registry()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool"));
        assert!(err.contains("nonexistent_tool"));
    }

    #[tokio::test]
    async fn execute_read_memory_missing_param() {
        let (_dir, memory) = setup();
        let result = execute_tool("read_memory", &serde_json::json!({}), &memory, None, None, None, &empty_registry()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("topic"));
    }

    #[tokio::test]
    async fn execute_write_memory_missing_content() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "write_memory",
            &serde_json::json!({"topic": "x"}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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

        let result = execute_tool(
            "complete_bootstrap",
            &serde_json::json!({}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "update_soul",
            &serde_json::json!({"content": new_soul}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
        let result = execute_tool(
            "update_soul",
            &serde_json::json!({}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
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
    async fn execute_get_recent_messages_empty_store() {
        let (_dir, memory) = setup();
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let store = crate::messages::MessageStore::init(&db_path).unwrap();

        let result = execute_tool(
            "get_recent_messages",
            &serde_json::json!({}),
            &memory,
            None,
            None,
            Some(&store),
            &empty_registry(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => assert!(s.contains("No messages")),
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_get_recent_messages_with_data() {
        let (_dir, memory) = setup();
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("messages.db");
        let store = crate::messages::MessageStore::init(&db_path).unwrap();
        store.log_inbound("telegram", "leo", "test message", "message").unwrap();
        store.log_outbound("telegram", "test response", None, Some(500), Some(10), Some(5)).unwrap();

        let result = execute_tool(
            "get_recent_messages",
            &serde_json::json!({"count": 10}),
            &memory,
            None,
            None,
            Some(&store),
            &empty_registry(),
        )
        .await
        .unwrap();
        match result {
            ToolResult::Immediate(s) => {
                assert!(s.contains("leo: test message"));
                assert!(s.contains("Nova: test response"));
            }
            _ => panic!("expected Immediate"),
        }
    }

    #[tokio::test]
    async fn execute_get_recent_messages_without_store() {
        let (_dir, memory) = setup();
        let result = execute_tool(
            "get_recent_messages",
            &serde_json::json!({}),
            &memory,
            None,
            None,
            None,
            &empty_registry(),
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Message store not available"));
    }
}
