use anyhow::Result;
use async_trait::async_trait;
use nv_core::ToolDefinition;
use serde_json::Value;

/// Dependency interface for daemon-coupled tools.
///
/// The standalone `nv-tools` binary has no implementor — it only exposes the
/// 16 stateless tools. When embedded inside `nv-daemon`, the daemon constructs
/// a concrete `DaemonSharedDeps` that wires into all live daemon resources.
///
/// The single `call_tool` method keeps the trait surface minimal. The daemon
/// implementation simply delegates to its existing `execute_tool_send`
/// dispatch function.
#[async_trait]
pub trait SharedDeps: Send + Sync {
    /// Return tool definitions for all daemon-coupled tools.
    ///
    /// This allows the `ToolRegistry` to advertise daemon tools in `tools/list`
    /// responses without knowing their schemas.
    fn daemon_tool_definitions(&self) -> Vec<ToolDefinition>;

    /// Dispatch a daemon-coupled tool call by name.
    ///
    /// Returns a JSON value that will be forwarded to the MCP caller as-is.
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value>;
}
