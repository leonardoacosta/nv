use serde::{Deserialize, Serialize};

/// Tool definition in the Anthropic API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Serialize to the Anthropic API wire format.
    ///
    /// Returns a JSON object with keys `name`, `description`, and `input_schema`
    /// matching the shape expected by the Anthropic Messages API `tools` array.
    pub fn anthropic_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_json_matches_wire_format() {
        let tool = ToolDefinition {
            name: "read_memory".into(),
            description: "Read a memory topic".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": {"type": "string", "description": "The memory topic"}
                },
                "required": ["topic"]
            }),
        };

        let json = tool.anthropic_json();

        assert_eq!(json["name"], "read_memory");
        assert_eq!(json["description"], "Read a memory topic");
        assert_eq!(json["input_schema"]["type"], "object");
        assert!(json["input_schema"]["properties"]["topic"].is_object());
        assert_eq!(json["input_schema"]["required"][0], "topic");

        // Keys must be snake_case (not camelCase)
        assert!(json.get("inputSchema").is_none());
    }

    #[test]
    fn anthropic_json_empty_schema() {
        let tool = ToolDefinition {
            name: "ping".into(),
            description: "No-op ping".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = tool.anthropic_json();
        assert_eq!(json["name"], "ping");
        assert_eq!(json["input_schema"]["type"], "object");
    }
}
