use anyhow::Result;
use nv_core::ToolDefinition;
use serde_json::Value;

/// Registry of available MCP tools.
pub struct ToolRegistry {
    tools: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Return all registered tools in MCP list format.
    pub fn list_tools(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            })
            .collect()
    }

    /// Dispatch a tool call by name.
    ///
    /// Returns an error result for any tool since no tools are registered yet.
    pub fn call_tool(&self, name: &str, _args: Value) -> Result<Value> {
        anyhow::bail!("Tool not found: {name}")
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
