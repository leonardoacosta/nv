use serde_json::Value;
use tracing::warn;

use crate::registry::ToolRegistry;

/// MCP server that dispatches JSON-RPC requests.
pub struct McpServer {
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
        }
    }

    /// Handle a single JSON-RPC request and return the response value.
    pub fn handle_request(&self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");

        match method {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, &request),
            other => {
                warn!(method = other, "unknown MCP method");
                self.error_response(id, -32601, "Method not found")
            }
        }
    }

    // ── Handlers ────────────────────────────────────────────────────

    fn handle_initialize(&self, id: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "nv-tools",
                    "version": "0.1.0"
                }
            }
        })
    }

    fn handle_tools_list(&self, id: Value) -> Value {
        let tools = self.registry.list_tools();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": tools
            }
        })
    }

    fn handle_tools_call(&self, id: Value, request: &Value) -> Value {
        let name = request
            .get("params")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");

        let args = request
            .get("params")
            .and_then(|p| p.get("arguments"))
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.registry.call_tool(name, args) {
            Ok(result) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            }),
            Err(_) => self.error_response(id, -32601, "Tool not found"),
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn error_response(&self, id: Value, code: i32, message: &str) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message
            }
        })
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
