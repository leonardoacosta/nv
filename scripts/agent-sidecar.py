#!/usr/bin/env python3
"""agent-sidecar.py — Long-running Agent SDK sidecar for the Nova daemon.

Protocol
--------
Reads newline-delimited JSON requests from stdin, one per line.
Writes newline-delimited JSON responses to stdout, one per line.
All log output goes to stderr.

Request schema:
    {
        "id": "<uuid>",
        "system": "<system prompt>",
        "prompt": "<user prompt>",
        "tools": [{"name": "...", "description": "...", "input_schema": {...}}, ...],
        "max_turns": 30,
        "timeout_secs": 300
    }

Response schema:
    {
        "id": "<uuid>",
        "content": [{"type": "text", "text": "..."}],
        "stop_reason": "end_turn" | "max_turns" | "error",
        "tool_calls": [{"name": "...", "input": {...}, "result": "..."}],
        "error": null | "<message>"
    }

Usage:
    python3 scripts/agent-sidecar.py          # production mode
    python3 scripts/agent-sidecar.py --test   # self-test, prints OK, exits 0
"""

from __future__ import annotations

import json
import logging
import os
import signal
import sys
import asyncio
from typing import Any

# ---------------------------------------------------------------------------
# Logging — goes to stderr so it doesn't pollute the stdout JSON stream
# ---------------------------------------------------------------------------
logging.basicConfig(
    stream=sys.stderr,
    level=logging.INFO,
    format="%(asctime)s [sidecar] %(levelname)s %(message)s",
    datefmt="%Y-%m-%dT%H:%M:%S",
)
logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Dependency check
# ---------------------------------------------------------------------------
def _check_sdk() -> bool:
    try:
        import claude_agent_sdk  # noqa: F401
        return True
    except ImportError:
        return False


# ---------------------------------------------------------------------------
# Graceful shutdown state
# ---------------------------------------------------------------------------
_shutdown_event: asyncio.Event | None = None
_in_flight_task: asyncio.Task | None = None


def _handle_sigterm(signum, frame):  # noqa: ANN001, ANN201
    """Signal handler: request graceful shutdown."""
    logger.info("SIGTERM received — initiating graceful shutdown")
    if _shutdown_event is not None:
        # Thread-safe set from signal handler
        asyncio.get_event_loop().call_soon_threadsafe(_shutdown_event.set)


# ---------------------------------------------------------------------------
# Core: process a single request
# ---------------------------------------------------------------------------
async def _process_request(req: dict[str, Any]) -> dict[str, Any]:
    """Call the Agent SDK for a single sidecar request and return a response dict."""
    from claude_agent_sdk import (
        ClaudeAgentOptions,
        AssistantMessage,
        ResultMessage,
        TextBlock,
        ToolUseBlock,
        query,
    )
    from nova_tools_mcp import build_mcp_server, allowed_tool_names

    req_id: str = req.get("id", "unknown")
    system_prompt: str = req.get("system", "")
    prompt: str = req.get("prompt", "")
    tool_defs: list[dict[str, Any]] = req.get("tools", [])
    max_turns: int = req.get("max_turns", 30)
    timeout_secs: float = req.get("timeout_secs", 300)

    logger.info("Processing request id=%s max_turns=%d timeout=%gs", req_id, max_turns, timeout_secs)

    # Build MCP server for this request's tool set
    mcp_servers: dict[str, Any] = {}
    allowed_tools: list[str] = []
    if tool_defs:
        mcp_servers["nova-tools"] = build_mcp_server(tool_defs)
        allowed_tools = allowed_tool_names(tool_defs)
        logger.debug("Registered %d tools: %s", len(tool_defs), [t["name"] for t in tool_defs])

    options = ClaudeAgentOptions(
        system_prompt=system_prompt if system_prompt else None,
        mcp_servers=mcp_servers,
        allowed_tools=allowed_tools,
        permission_mode="bypassPermissions",
        max_turns=max_turns,
    )

    # Sandbox: use auth-only Claude sandbox when available
    sandbox_claude = os.path.expanduser("~/.nv/claude-sandbox/.claude")
    if os.path.isdir(sandbox_claude):
        options.env = {**options.env, "HOME": os.path.expanduser("~/.nv/claude-sandbox")}

    content_blocks: list[dict[str, Any]] = []
    tool_calls: list[dict[str, Any]] = []
    stop_reason = "end_turn"

    try:
        async with asyncio.timeout(timeout_secs):
            async for message in query(prompt=prompt, options=options):
                if isinstance(message, AssistantMessage):
                    for block in message.content:
                        if isinstance(block, TextBlock):
                            content_blocks.append({"type": "text", "text": block.text})
                        elif isinstance(block, ToolUseBlock):
                            tool_calls.append({
                                "name": block.name,
                                "input": block.input,
                            })
                            logger.debug("Tool used: %s", block.name)
                elif isinstance(message, ResultMessage):
                    if message.stop_reason:
                        stop_reason = message.stop_reason
                    logger.info(
                        "Request id=%s done stop_reason=%s cost=$%.6f",
                        req_id,
                        stop_reason,
                        message.total_cost_usd or 0.0,
                    )
    except TimeoutError:
        logger.warning("Request id=%s timed out after %gs", req_id, timeout_secs)
        stop_reason = "error"
        return {
            "id": req_id,
            "content": content_blocks,
            "stop_reason": stop_reason,
            "tool_calls": tool_calls,
            "error": f"Request timed out after {timeout_secs}s",
        }
    except Exception as exc:  # noqa: BLE001
        logger.error("Request id=%s failed: %s", req_id, exc, exc_info=True)
        return {
            "id": req_id,
            "content": content_blocks,
            "stop_reason": "error",
            "tool_calls": tool_calls,
            "error": str(exc),
        }

    return {
        "id": req_id,
        "content": content_blocks,
        "stop_reason": stop_reason,
        "tool_calls": tool_calls,
        "error": None,
    }


# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------
async def _main_loop() -> None:
    global _shutdown_event, _in_flight_task

    _shutdown_event = asyncio.Event()

    # Register SIGTERM handler inside the event loop
    loop = asyncio.get_running_loop()
    loop.add_signal_handler(signal.SIGTERM, lambda: _shutdown_event.set())

    logger.info("Nova agent sidecar ready — waiting for requests on stdin")

    stdin_reader = asyncio.StreamReader()
    transport, protocol = await loop.connect_read_pipe(
        lambda: asyncio.StreamReaderProtocol(stdin_reader), sys.stdin.buffer
    )

    try:
        while not _shutdown_event.is_set():
            # Race: either a line arrives on stdin, or shutdown is requested
            read_task = asyncio.create_task(stdin_reader.readline())
            shutdown_task = asyncio.create_task(_shutdown_event.wait())

            done, pending = await asyncio.wait(
                {read_task, shutdown_task},
                return_when=asyncio.FIRST_COMPLETED,
            )

            for t in pending:
                t.cancel()

            if shutdown_task in done:
                # Shutdown requested — cancel any pending read
                logger.info("Shutdown event set, exiting main loop")
                break

            line = read_task.result()
            if not line:
                # EOF on stdin — daemon closed the pipe
                logger.info("stdin EOF — exiting")
                break

            line = line.decode("utf-8", errors="replace").strip()
            if not line:
                continue

            # Parse request
            try:
                req = json.loads(line)
            except json.JSONDecodeError as exc:
                logger.error("Bad JSON on stdin: %s — %s", line[:200], exc)
                continue

            # Process request (cancellable on shutdown)
            _in_flight_task = asyncio.create_task(_process_request(req))
            try:
                response = await _in_flight_task
            except asyncio.CancelledError:
                logger.info("In-flight request id=%s cancelled", req.get("id", "?"))
                response = {
                    "id": req.get("id", "unknown"),
                    "content": [],
                    "stop_reason": "error",
                    "tool_calls": [],
                    "error": "Request cancelled due to shutdown",
                }
            finally:
                _in_flight_task = None

            # Write response to stdout — flush immediately
            line_out = json.dumps(response, ensure_ascii=False)
            sys.stdout.write(line_out + "\n")
            sys.stdout.flush()

    finally:
        transport.close()

    # Cancel any in-flight task on the way out
    if _in_flight_task is not None and not _in_flight_task.done():
        _in_flight_task.cancel()
        try:
            await _in_flight_task
        except asyncio.CancelledError:
            pass

    sys.stdout.flush()
    logger.info("Sidecar exited cleanly")


# ---------------------------------------------------------------------------
# Self-test mode
# ---------------------------------------------------------------------------
def _run_self_test() -> None:
    """Verify SDK import and MCP registration work, then print OK."""
    if not _check_sdk():
        print("ERROR: claude-agent-sdk is not installed. Run: pipx install claude-agent-sdk", file=sys.stderr)
        sys.exit(1)

    # Verify nova_tools_mcp is importable (scripts/ dir must be on sys.path)
    scripts_dir = os.path.dirname(os.path.abspath(__file__))
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)

    try:
        from nova_tools_mcp import build_mcp_server, allowed_tool_names  # noqa: F401
    except ImportError as exc:
        print(f"ERROR: Cannot import nova_tools_mcp: {exc}", file=sys.stderr)
        sys.exit(1)

    # Register a dummy tool to ensure create_sdk_mcp_server works
    dummy_tool_def = [{
        "name": "test_tool",
        "description": "Sidecar self-test tool",
        "input_schema": {
            "type": "object",
            "properties": {"msg": {"type": "string"}},
            "required": ["msg"],
        },
    }]
    try:
        mcp = build_mcp_server(dummy_tool_def)
        assert mcp is not None, "build_mcp_server returned None"
        names = allowed_tool_names(dummy_tool_def)
        assert names == ["mcp__nova-tools__test_tool"], f"Unexpected names: {names}"
    except Exception as exc:  # noqa: BLE001
        print(f"ERROR: MCP registration failed: {exc}", file=sys.stderr)
        sys.exit(1)

    print("OK")
    sys.exit(0)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
def main() -> None:
    if not _check_sdk():
        print(
            "ERROR: claude-agent-sdk is not installed.\n"
            "Run: pipx install claude-agent-sdk",
            file=sys.stderr,
        )
        sys.exit(1)

    # Add scripts/ directory to sys.path so we can import nova_tools_mcp
    scripts_dir = os.path.dirname(os.path.abspath(__file__))
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)

    if len(sys.argv) > 1 and sys.argv[1] == "--test":
        _run_self_test()
        return  # unreachable — _run_self_test always exits

    try:
        asyncio.run(_main_loop())
    except KeyboardInterrupt:
        logger.info("KeyboardInterrupt — exiting")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
