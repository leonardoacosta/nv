"""nova-tools-mcp.py — Dynamic MCP server bridge for Nova tool calls.

Registers tool definitions from a sidecar request and forwards each
Claude tool call to the daemon's HTTP endpoint at
http://127.0.0.1:8400/api/tool-call.

Imported as a module by agent-sidecar.py — not run as a standalone process.
"""

import json
import logging
import urllib.request
import urllib.error
from typing import Any

logger = logging.getLogger(__name__)

DAEMON_TOOL_CALL_URL = "http://127.0.0.1:8400/api/tool-call"
TIMEOUT_SECS = 30


def _http_post(url: str, payload: dict[str, Any], timeout: float = TIMEOUT_SECS) -> dict[str, Any]:
    """Send a JSON POST and return the parsed response body."""
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"HTTP {exc.code} from daemon: {body}") from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"Cannot reach daemon at {url}: {exc.reason}") from exc


def build_mcp_server(tool_defs: list[dict[str, Any]]):  # noqa: ANN201
    """Build and return a McpSdkServerConfig from a list of tool definitions.

    Each entry in *tool_defs* must have the shape expected by the Nova daemon:
        {
            "name": "tool_name",
            "description": "...",
            "input_schema": { "type": "object", "properties": {...}, ... }
        }

    Returns a ``McpSdkServerConfig`` ready to pass as a value in
    ``ClaudeAgentOptions.mcp_servers``.
    """
    try:
        from claude_agent_sdk import create_sdk_mcp_server, SdkMcpTool
    except ImportError as exc:
        raise RuntimeError(
            "claude-agent-sdk is not installed. "
            "Run: pipx install claude-agent-sdk"
        ) from exc

    sdk_tools: list[SdkMcpTool] = []

    for defn in tool_defs:
        tool_name: str = defn["name"]
        tool_description: str = defn.get("description", "")
        input_schema: dict[str, Any] = defn.get("input_schema", {"type": "object", "properties": {}})

        # Build a closure that captures tool_name (avoid late-binding issues).
        def _make_handler(name: str):
            async def handler(args: dict[str, Any]) -> dict[str, Any]:
                logger.debug("Tool call: %s %r", name, args)
                try:
                    result = _http_post(
                        DAEMON_TOOL_CALL_URL,
                        {"tool_name": name, "input": args},
                    )
                except RuntimeError as exc:
                    logger.error("Tool %s failed: %s", name, exc)
                    return {
                        "content": [{"type": "text", "text": str(exc)}],
                        "is_error": True,
                    }

                if result.get("error"):
                    return {
                        "content": [{"type": "text", "text": result["error"]}],
                        "is_error": True,
                    }

                tool_result = result.get("result", "")
                return {"content": [{"type": "text", "text": str(tool_result)}]}

            return handler

        sdk_tools.append(
            SdkMcpTool(
                name=tool_name,
                description=tool_description,
                input_schema=input_schema,
                handler=_make_handler(tool_name),
            )
        )

    return create_sdk_mcp_server(
        name="nova-tools",
        version="1.0.0",
        tools=sdk_tools if sdk_tools else None,
    )


def allowed_tool_names(tool_defs: list[dict[str, Any]]) -> list[str]:
    """Return the MCP-qualified tool names Claude needs in allowed_tools.

    The SDK uses the pattern ``mcp__<server_name>__<tool_name>``.
    """
    return [f"mcp__nova-tools__{defn['name']}" for defn in tool_defs]
