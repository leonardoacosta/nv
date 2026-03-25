use std::sync::Arc;

use anyhow::Result;
use nv_core::ToolDefinition;
use serde_json::Value;

use crate::dispatch::{dispatch_stateless, stateless_tool_definitions};
use crate::shared::SharedDeps;

/// Registry of available MCP tools.
///
/// Stateless tools (handled locally in nv-tools) are resolved through
/// `dispatch_stateless`. Daemon-coupled tools are listed and dispatched via
/// the optional `SharedDeps` implementation — present when running inside
/// `nv-daemon`, absent when the standalone `nv-tools` binary runs without
/// a daemon.
pub struct ToolRegistry {
    stateless_tools: Vec<ToolDefinition>,
    shared: Option<Arc<dyn SharedDeps>>,
}

impl ToolRegistry {
    /// Create a registry with all stateless tools loaded (standalone mode).
    pub fn new() -> Self {
        Self {
            stateless_tools: stateless_tool_definitions(),
            shared: None,
        }
    }

    /// Create a registry with stateless tools and a daemon `SharedDeps` handle.
    pub fn with_shared(shared: Arc<dyn SharedDeps>) -> Self {
        Self {
            stateless_tools: stateless_tool_definitions(),
            shared: Some(shared),
        }
    }

    /// Return all registered tools in MCP `tools/list` format.
    ///
    /// Includes both stateless tools and daemon-coupled tools (when SharedDeps
    /// is present).
    pub fn list_tools(&self) -> Vec<Value> {
        let mut result: Vec<Value> = self
            .stateless_tools
            .iter()
            .map(tool_to_json)
            .collect();

        if let Some(shared) = &self.shared {
            result.extend(
                shared
                    .daemon_tool_definitions()
                    .iter()
                    .map(tool_to_json),
            );
        }

        result
    }

    /// Dispatch a tool call by name.
    ///
    /// Routes to either the local stateless dispatch or the daemon `SharedDeps`
    /// implementation. Returns an error if the tool is not found in either.
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        // Check stateless tools first
        let is_stateless = self.stateless_tools.iter().any(|t| t.name == name);
        if is_stateless {
            return dispatch_stateless(name, &args).await;
        }

        // Delegate to daemon SharedDeps
        if let Some(shared) = &self.shared {
            return shared.call_tool(name, args).await;
        }

        anyhow::bail!("Tool not found: {name}")
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn tool_to_json(t: &ToolDefinition) -> Value {
    serde_json::json!({
        "name": t.name,
        "description": t.description,
        "inputSchema": t.input_schema,
    })
}
