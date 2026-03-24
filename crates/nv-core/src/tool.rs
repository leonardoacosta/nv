use serde::{Deserialize, Serialize};

/// Tool definition in the Anthropic API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
