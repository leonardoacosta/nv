# Proposal: Add Resend Tools

## Change ID
`add-resend-tools`

## Summary

Resend email delivery status tools via REST API. Two read-only tools (`resend_emails`,
`resend_bounces`) that query the Resend API for email delivery status and bounce data,
enabling Nova to report on email health across all sending domains.

## Context
- Extends: `crates/nv-daemon/src/tools.rs` (tool definitions + dispatch), `crates/nv-daemon/src/agent.rs` (tool execution)
- Related: Existing tool pattern (Jira, Nexus, Memory tools), `add-tool-audit-log` spec (audit logging)
- PRD ref: Phase 2, Section 6.1 — Tier 3 (API key)

## Motivation

Resend handles transactional email for multiple projects (OO, TC, etc.). Currently there's
no visibility into delivery status, bounces, or failures without logging into the Resend
dashboard. Wiring Resend into Nova lets Leo ask "Any email bounces today?" or "What's the
delivery status for OO?" from Telegram.

## Requirements

### Req-1: HTTP Client Module

New file `crates/nv-daemon/src/resend.rs` with:
- `ResendClient` struct holding API key and reqwest client
- Base URL: `https://api.resend.com`
- Auth: `Authorization: Bearer $RESEND_API_KEY` header
- All requests are GET (read-only)

### Req-2: resend_emails Tool

`resend_emails(status)` — List recent emails, optionally filtered by delivery status.

- Endpoint: `GET /emails` (list emails)
- Input: `status` (optional) — filter by `"delivered"`, `"bounced"`, `"complained"`, or omit for all
- Output: Formatted list of recent emails with to, subject, status, created_at
- Limit: return last 20 emails max

### Req-3: resend_bounces Tool

`resend_bounces()` — List emails that bounced.

- Filters the emails endpoint for `status = "bounced"`
- Output: Formatted list with to address, subject, bounce reason, timestamp
- If no bounces: return "No bounces found"

### Req-4: Tool Registration

Register both tools in `register_tools()` with Anthropic tool schema format.
Wire dispatch in `execute_tool()` to call ResendClient methods.

### Req-5: Configuration

- Env var: `RESEND_API_KEY` — loaded from env file at daemon startup
- Add to `NvConfig` struct if centralized config exists, or read from env directly
- Fail gracefully: if API key is missing, tools return "Resend not configured" instead of crashing

### Req-6: Audit Logging

Every tool invocation logged via tool audit log (depends on `add-tool-audit-log`).
Log: tool name, input summary, success/failure, duration_ms.

## Scope
- **IN**: ResendClient HTTP module, resend_emails tool, resend_bounces tool, tool registration, env config
- **OUT**: Sending emails, domain management, webhook handling, audience/contact management

## Impact
| Area | Change |
|------|--------|
| `crates/nv-daemon/src/resend.rs` | New: ResendClient with list_emails(), list_bounces() |
| `crates/nv-daemon/src/tools.rs` | Add 2 tool definitions + dispatch cases |
| `crates/nv-daemon/src/main.rs` | Init ResendClient, pass to tool executor |
| `config/env` or `.env` | Add RESEND_API_KEY |

## Risks
| Risk | Mitigation |
|------|-----------|
| Resend API rate limits | Read-only, low frequency. Add retry with backoff if 429. |
| API key leaked in logs | Never log API key. Log only sanitized request URLs. |
| API response format changes | Pin to v1 API. Deserialize with serde, handle unknown fields gracefully. |
