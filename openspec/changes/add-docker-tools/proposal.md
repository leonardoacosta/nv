# Proposal: Docker Tools

## Change ID
`add-docker-tools`

## Summary

Docker container health monitoring via unix socket. Two tools: `docker_status()` returns running
containers with state/uptime/ports, `docker_logs(container, lines)` returns recent log lines.
All invocations logged to the tool_usage audit table.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (register_tools, execute_tool)
- Depends on: `add-tool-audit-log` (tool_usage table for logging)
- Related: PRD §6.1 (Individual Tools — Tier 1: Zero Auth)

## Motivation

Nova runs on a homelab with 10+ Docker containers (Tailscale, Home Assistant, PostgreSQL, Redis,
monitoring, etc.). Currently, checking container status requires SSH + `docker ps`. Docker tools
let Nova answer "are my containers healthy?" and "what's in the Tailscale logs?" directly from
Telegram.

Benefits:
1. **Zero auth** — Docker socket requires no API keys or tokens
2. **Homelab visibility** — container health is the foundation for `homelab_status()` aggregation
3. **Fast** — unix socket calls return in <50ms
4. **Debugging** — `docker_logs` enables quick triage of container issues from mobile

## Requirements

### Req-1: Docker Client Module

Create `crates/nv-daemon/src/docker.rs` with a `DockerClient` that communicates via the unix
socket at `/var/run/docker.sock` using `reqwest` with a unix socket connector (or `hyper` with
`hyperlocal`). The client makes HTTP requests to the Docker Engine API.

### Req-2: docker_status Tool

`GET /containers/json` from the Docker API. Return a summary for each container:
- Name (stripped leading `/`)
- Image
- State (running/stopped/restarting)
- Status string (e.g., "Up 3 days")
- Ports (host:container mappings)

Format as a concise text table for Claude to relay. Limit to running containers by default;
accept an optional `all` parameter to include stopped containers.

### Req-3: docker_logs Tool

`GET /containers/{id}/logs?stdout=true&stderr=true&tail={lines}` from the Docker API.
Parameters:
- `container`: container name or ID (required)
- `lines`: number of log lines to return (default 50, max 200)

Strip Docker log frame headers (8-byte prefix per line). Return raw log text.

### Req-4: Tool Registration

Register both tools in `register_tools()`:
- `docker_status(all?)` — list containers with health info
- `docker_logs(container, lines?)` — recent logs from a specific container

### Req-5: Audit Logging

After each tool invocation, log to `tool_usage` table via `MessageStore::log_tool_usage()`:
- tool_name: "docker_status" or "docker_logs"
- input_summary: parameters passed
- result_summary: truncated response (container count or first line)
- duration_ms: execution time
- success: based on HTTP status / parse success

## Scope
- **IN**: Docker socket client, docker_status, docker_logs, tool registration, audit logging
- **OUT**: Container management (start/stop/restart), image management, compose operations, Docker Compose file parsing

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/docker.rs` | New module: DockerClient with unix socket HTTP, docker_status(), docker_logs() |
| `crates/nv-daemon/src/tools.rs` | Register docker_status and docker_logs tools, add execution handlers |
| `crates/nv-daemon/src/main.rs` | Add `mod docker;` declaration |
| `Cargo.toml` | Add `hyperlocal` or equivalent unix socket HTTP crate (if not using reqwest unix feature) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Docker socket not available (permission denied) | Check socket exists + readable on startup; disable tools if unavailable; log warning |
| Docker API version mismatch | Use `/v1.43/` versioned API path; Docker maintains backward compat |
| Large log output exceeds Claude context | Cap lines at 200; truncate total output to 10KB |
| Container names with special characters | URL-encode container name in API path |
