# Proposal: Jira Webhooks

## Change ID
`jira-webhooks`

## Summary
Inbound webhook handler for bidirectional Jira sync. Adds an HTTP endpoint to the existing axum server that receives Jira webhook payloads, validates their authenticity, parses issue and comment events, updates Nova's memory with external changes, and alerts via Telegram when tracked issues change.

## Context
- Extends: `crates/nv-daemon/src/http.rs` (existing axum server), `crates/nv-daemon/src/main.rs` (route registration)
- Related: Existing Jira integration (`crates/nv-daemon/src/jira/`) handles outbound operations (create, transition, comment); this spec adds the inbound direction

## Motivation
Nova's current Jira integration is one-way: Nova creates issues, transitions them, and adds comments. But when teammates update issues externally (reassign, comment, change status), Nova is blind to those changes until the next polling cycle or manual check. Webhooks provide real-time notification of external changes, enabling Nova to:

1. **Stay current** — memory reflects the latest issue state without polling
2. **Alert proactively** — notify via Telegram when tracked issues change
3. **Close the loop** — bidirectional sync means Nova's view of Jira is always accurate

## Requirements

### Req-1: Jira Webhook Payload Types
Define Rust types with serde for Jira webhook payloads:
- `WebhookEvent` — top-level envelope with `webhookEvent` (event name string), `timestamp`, and event-specific payload
- `IssueEvent` — contains `issue` (key, summary, status, assignee, priority) and `changelog` (items with field, fromString, toString)
- `CommentEvent` — contains `issue` (key, summary) and `comment` (author, body, created)

### Req-2: HTTP Endpoint
Add `POST /webhooks/jira` route to the existing axum server in http.rs. The handler:
1. Validates the webhook secret (Req-3)
2. Deserializes the JSON body into `WebhookEvent`
3. Routes to event-specific handlers based on `webhookEvent` field
4. Returns 200 OK on success, 401 on auth failure, 400 on parse failure

### Req-3: Webhook Secret Validation
Validate inbound webhooks using a shared secret. Jira sends the secret as a query parameter or header (configurable). Compare against `jira_webhook_secret` from config. Reject with 401 if missing or mismatched.

### Req-4: Event Parsing and Routing
Handle these Jira webhook event types:
- `jira:issue_updated` — extract changed fields from changelog, determine if the change is relevant (status, assignee, priority changes)
- `jira:issue_created` — capture new issue details
- `comment_created` — extract comment author and body

### Req-5: Memory Update
On relevant events, update Nova's memory (`~/.nv/memory/`) with the latest issue state. Write or update a memory entry reflecting the external change so the agent loop has current context on next trigger.

### Req-6: Telegram Alert
On relevant events, send a formatted Telegram message via the existing TelegramClient:
- Issue updated: "{KEY} status changed: {from} -> {to} by {actor}"
- Issue created: "New issue {KEY}: {summary} (by {actor})"
- Comment added: "{actor} commented on {KEY}: {preview}"

### Req-7: Configuration
New fields in `nv.toml`:
- `jira_webhook_secret` (String) — shared secret for webhook validation
- No separate `[jira-webhooks]` section needed; add to existing `[jira]` section

## Scope
- **IN**: Webhook payload types, axum route, secret validation, event parsing (issue_updated, issue_created, comment_created), memory update, Telegram alert, config, tests
- **OUT**: Webhook registration automation (manual setup in Jira admin), webhook retry/deduplication (Jira handles retries), issue_deleted events, sprint/board events, attachment events

## Impact
| Area | Change |
|------|--------|
| crates/nv-daemon/src/jira/webhooks.rs | New: webhook types, handler, event routing |
| crates/nv-daemon/src/http.rs | Add POST /webhooks/jira route |
| crates/nv-daemon/src/jira/mod.rs | Re-export webhooks module |
| crates/nv-core/src/config.rs | Add jira_webhook_secret to JiraConfig |
| crates/nv-daemon/src/main.rs | Wire webhook route into axum router |

## Risks
| Risk | Mitigation |
|------|-----------|
| Webhook flood from busy Jira project | Filter to tracked issues only; rate-limit Telegram alerts (debounce per issue, 1 alert/min) |
| Shared secret not cryptographically strong | Document: use a long random string (32+ chars); upgrade to HMAC if Jira supports it |
| Jira payload format varies by version/plugin | Deserialize with serde defaults and Option fields; log unparseable payloads without crashing |
| Memory file conflicts with agent loop writes | Append-only memory entries with timestamps; agent loop reads are snapshot-based |
| Webhook endpoint exposed to internet | Require Tailscale or reverse proxy; secret validation is defense-in-depth |
