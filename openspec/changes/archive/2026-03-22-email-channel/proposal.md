# Proposal: Email Channel

## Change ID
`email-channel`

## Summary
Native email channel adapter via MS Graph API (Outlook). Reuses the OAuth2 token infrastructure from the teams-channel spec (shared MsGraphClient). Polls a configurable set of mail folders on interval, extracts text from HTML email bodies, and supports reply-with-confirmation through the PendingAction flow. Implements the existing `Channel` trait from nv-core.

## Context
- Extends: `crates/nv-core/src/channel.rs` (Channel trait), `crates/nv-daemon/src/main.rs` (channel registration)
- Depends on: `teams-channel` spec for shared MsGraphClient and OAuth2 token refresh
- Related: MS Graph mail endpoints (GET /me/messages, POST /me/sendMail), existing PendingAction system for confirm-before-act

## Motivation
Email is a primary communication channel for work. Nova currently has no visibility into email — important messages sit unread until manually checked. A native email adapter lets Nova poll Outlook, triage incoming mail alongside Telegram/Discord/Teams messages, and optionally reply with confirmation. Reusing the MS Graph OAuth from the teams-channel spec means zero additional auth setup.

## Requirements

### Req-1: MsGraphMailClient
Mail-specific client methods on the shared MsGraphClient:
- `get_messages(folder_id: Option<&str>, after: &str, top: u32)` — GET `/me/mailFolders/{folder}/messages` with `$filter=receivedDateTime gt {after}` and `$top`
- `send_mail(to: &str, subject: &str, body: &str, reply_to_message_id: Option<&str>)` — POST `/me/sendMail` or POST `/me/messages/{id}/reply`

### Req-2: OAuth2 Token Reuse
Email channel shares the MsGraphClient instance from teams-channel. The existing OAuth2 app registration (tenant ID, client ID, client secret) and token refresh logic are reused — no new credentials. Mail permissions (Mail.Read, Mail.Send) are added to the same app registration scope.

### Req-3: EmailChannel (Channel Trait)
Implement the `Channel` trait for `EmailChannel`. On each poll tick, fetch new messages from configured folders, filter by sender/subject rules, convert to `InboundMessage` with extracted text body, and emit onto the trigger mpsc channel.

### Req-4: Mailbox Polling
A tokio task polls configured mail folders on interval (`email_poll_interval_secs`, default: 60). Tracks the last seen receivedDateTime per folder. Backs off on consecutive errors.

### Req-5: Sender and Subject Filtering
Config-driven filtering to avoid noise:
- `sender_filter` — list of email addresses or domains to include (empty = all)
- `subject_filter` — list of subject substrings to include (empty = all)
- `folder_ids` — list of mail folder IDs to poll (default: Inbox)

### Req-6: HTML to Text Extraction
Email bodies are typically HTML. Extract plain text for the agent loop:
- Strip HTML tags, decode entities
- Preserve paragraph breaks
- Use a lightweight approach (regex or a small HTML parser like `scraper` crate)

### Req-7: Reply with Confirmation
When the agent determines a reply is appropriate, it creates a PendingAction (existing system). On approval via Telegram callback, the reply is sent via `send_mail` with the original message ID for proper threading.

### Req-8: Configuration
New `[email]` section in `nv.toml`:
- `enabled` (bool, default: false)
- `poll_interval_secs` (u64, default: 60)
- `folder_ids` (Vec<String>, default: ["Inbox"])
- `sender_filter` (Vec<String>, default: [])
- `subject_filter` (Vec<String>, default: [])

## Scope
- **IN**: MS Graph mail client methods, OAuth2 reuse from teams-channel, Channel trait implementation, polling loop, sender/subject filtering, HTML-to-text extraction, reply via PendingAction, config section, daemon wiring, unit tests
- **OUT**: IMAP fallback (MS Graph only for now), attachment processing, calendar invites, shared mailboxes, webhook subscriptions (polling only), draft management

## Impact
| Area | Change |
|------|--------|
| crates/nv-daemon/src/email/mod.rs | New module: MsGraphMailClient methods, email types |
| crates/nv-daemon/src/email/channel.rs | EmailChannel implementing Channel trait |
| crates/nv-daemon/src/email/html.rs | HTML-to-text extraction utility |
| crates/nv-daemon/src/msgraph.rs | Shared MsGraphClient gains mail methods (or email module imports shared client) |
| crates/nv-core/src/config.rs | Add EmailConfig to DaemonConfig |
| crates/nv-daemon/src/main.rs | Spawn email polling task if enabled, wire to MsGraphClient |

## Risks
| Risk | Mitigation |
|------|-----------|
| MS Graph rate limits (throttling 429s) | Respect Retry-After header, default poll interval is conservative (60s) |
| OAuth scope expansion requires admin consent | Document required permissions; same app registration as Teams |
| HTML parsing edge cases (malformed email) | Fallback to raw text content-type if available; log and skip unparseable bodies |
| High email volume floods agent loop | Sender/subject filters + folder scoping reduce noise; configurable |
| Reply threading breaks on forwarded emails | Use in-reply-to message ID; degrade gracefully to new message |
