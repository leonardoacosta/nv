# Proposal: Add Home Assistant Tools

## Change ID
`add-ha-tools`

## Summary

Home Assistant integration via REST API on localhost:8123. Three tools: `ha_states` (list all
entity states), `ha_entity` (get specific entity), and `ha_service_call` (invoke HA services
with PendingAction confirmation). Enables Nova to monitor and control smart home devices.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution, PendingAction)
- Related: Existing tool pattern, `add-tool-audit-log` spec, PendingAction confirmation flow (used for write operations)
- PRD ref: Phase 2, Section 6.1 — Tier 4 (Special)

## Motivation

Home Assistant runs on the homelab managing lights, sensors, climate, and automations. Currently
Leo must open the HA dashboard or app to check/control devices. Wiring HA into Nova lets Leo
say "Turn off the office lights" or "What's the temperature in the living room?" via Telegram.
The `service_call` tool requires confirmation because it performs physical actions.

## Requirements

### Req-1: HTTP Client Module

New file `crates/nv-daemon/src/homeassistant.rs` with:
- `HAClient` struct holding base URL and long-lived access token
- Base URL: `http://localhost:8123` (configurable via env)
- Auth: `Authorization: Bearer $HA_TOKEN` header on all requests
- All GET requests are read-only. POST to `/api/services/*` is a write operation.

### Req-2: ha_states Tool

`ha_states()` — List all entity states.

- Endpoint: `GET /api/states`
- Output: Formatted summary grouped by domain (light, sensor, switch, climate, etc.)
- Include: entity_id, state, last_changed
- Cap output: show counts per domain + top 20 most recently changed entities

### Req-3: ha_entity Tool

`ha_entity(id)` — Get detailed state for a specific entity.

- Endpoint: `GET /api/states/<entity_id>`
- Input: `id` (required) — full entity ID (e.g., `"light.office"`, `"sensor.living_room_temperature"`)
- Output: Entity state, attributes (brightness, temperature, etc.), last_changed, last_updated

### Req-4: ha_service_call Tool (with PendingAction)

`ha_service_call(domain, service, data)` — Call a Home Assistant service.

- Endpoint: `POST /api/services/<domain>/<service>` with JSON body `data`
- Input:
  - `domain` (required) — e.g., `"light"`, `"switch"`, `"climate"`
  - `service` (required) — e.g., `"turn_on"`, `"turn_off"`, `"set_temperature"`
  - `data` (required) — JSON object with service data (e.g., `{"entity_id": "light.office"}`)
- **PendingAction**: This tool MUST trigger the confirmation flow before execution.
  Nova sends "I'm about to: Turn off light.office. Confirm?" with inline keyboard [Confirm] [Cancel].
  Only executes after user confirms.
- Output: Service call result or error message

### Req-5: Tool Registration

Register all 3 tools in `register_tools()`. Mark `ha_service_call` as requiring confirmation
in the tool metadata or dispatch logic.

### Req-6: Configuration

- Env vars: `HA_URL` (default `http://localhost:8123`) + `HA_TOKEN` (long-lived access token)
- Fail gracefully: if token missing, tools return "Home Assistant not configured"
- HA_TOKEN is machine-local (not in Doppler) per DEPLOY.md local-only rules

### Req-7: Audit Logging

Every tool invocation logged via tool audit log. For service_call: log domain, service, confirmation status, result.

## Scope
- **IN**: HAClient HTTP module, ha_states tool, ha_entity tool, ha_service_call tool with PendingAction, tool registration, env config
- **OUT**: Webhook/event subscriptions, automation management, HA add-on management, Lovelace dashboard config

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/homeassistant.rs` | New: HAClient with states(), entity(id), service_call(domain, service, data) |
| `crates/nv-daemon/src/tools.rs` | Add 3 tool definitions + dispatch cases (service_call with PendingAction) |
| `crates/nv-daemon/src/main.rs` | Init HAClient, pass to tool executor |
| `config/env` or `.env` | Add HA_URL, HA_TOKEN |

## Risks
| Risk | Mitigation |
|------|-----------|
| Accidental physical actions | PendingAction confirmation on all service_call invocations. No auto-confirm. |
| HA unreachable (network) | 5s timeout. Return "Home Assistant unreachable" on connection error. |
| Large entity list | Cap ha_states output at 20 entities + domain counts. Full list via ha_entity per-entity. |
| Token is long-lived | Store in local .env (not repo). Rotate via HA dashboard periodically. |
