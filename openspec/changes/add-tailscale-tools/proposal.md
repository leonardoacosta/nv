# Proposal: Tailscale Tools

## Change ID
`add-tailscale-tools`

## Summary

Tailscale network topology via `docker exec tailscale tailscale status --json`. Two tools:
`tailscale_status()` returns all nodes with online/offline state, IPs, and OS,
`tailscale_node(name)` returns detailed info for a specific node. All invocations logged to the
tool_usage audit table.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (register_tools, execute_tool)
- Depends on: `add-tool-audit-log` (tool_usage table for logging)
- Related: PRD §6.1 (Individual Tools — Tier 1: Zero Auth)

## Motivation

Nova's homelab spans multiple machines connected via Tailscale. Checking which nodes are online,
their IPs, and connectivity currently requires SSH + `tailscale status`. Tailscale tools let Nova
answer "is the Mac online?" or "what's the Tailscale IP of the NAS?" directly from Telegram.

Benefits:
1. **Zero auth** — runs via `docker exec` on the local Tailscale container, no API keys
2. **Network visibility** — node status is foundational for `homelab_status()` aggregation
3. **Fast** — CLI call returns in <500ms
4. **Troubleshooting** — "why can't I reach the NAS?" → check if node is online + last seen

## Requirements

### Req-1: Tailscale Client Module

Create `crates/nv-daemon/src/tailscale.rs` with a `TailscaleClient` that executes
`docker exec tailscale tailscale status --json` via `tokio::process::Command`. Parse the JSON
output into Rust structs.

### Req-2: tailscale_status Tool

Parse the `--json` output which contains:
- `Self`: the current node
- `Peer`: map of peer nodes with fields: HostName, DNSName, OS, Online, TailscaleIPs, LastSeen,
  Active, Relay

Return a summary table of all nodes:
- Hostname
- Online status (true/false)
- Tailscale IP (first IPv4)
- OS
- Last seen (if offline)

Format as concise text for Claude. Sort: online nodes first, then offline.

### Req-3: tailscale_node Tool

Accept a `name` parameter (hostname, case-insensitive match). Return detailed info for that node:
- Hostname, DNSName
- Online, Active
- All TailscaleIPs
- OS
- Relay (DERP server)
- Last seen timestamp
- Connection type (direct or relayed)

Return error if node not found.

### Req-4: Tool Registration

Register both tools in `register_tools()`:
- `tailscale_status()` — all nodes with online state
- `tailscale_node(name)` — detailed info for one node

### Req-5: Audit Logging

After each invocation, log to `tool_usage` table:
- tool_name: "tailscale_status" or "tailscale_node"
- input_summary: parameters (node name if applicable)
- result_summary: node count or node hostname
- duration_ms: execution time
- success: based on command exit code + parse success

## Scope
- **IN**: docker exec CLI execution, status parsing, node lookup, tool registration, audit logging
- **OUT**: Tailscale API (control plane), ACL management, route advertisement, exit nodes

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/tailscale.rs` | New module: TailscaleClient with docker exec command, JSON parsing, status() and node() methods |
| `crates/nv-daemon/src/tools.rs` | Register tailscale_status and tailscale_node tools, add execution handlers |
| `crates/nv-daemon/src/main.rs` | Add `mod tailscale;` declaration |

## Risks
| Risk | Mitigation |
|------|-----------|
| Tailscale container not running | Check container exists via `docker inspect tailscale` on startup; disable tools if unavailable |
| docker exec requires docker group membership | Nova daemon user must be in docker group (documented in deploy spec) |
| JSON output format changes across Tailscale versions | Pin to known fields; ignore unknown fields via serde(deny_unknown_fields = false) |
| Slow CLI execution (>1s) | 5s timeout on Command; cache status for 30s to avoid repeated calls |
