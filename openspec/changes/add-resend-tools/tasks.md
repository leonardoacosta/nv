# Implementation Tasks

<!-- beads:epic:TBD -->

## Rust Implementation

- [ ] [1.1] [P-1] Create crates/nv-daemon/src/resend.rs — ResendClient struct with reqwest::Client + api_key field, new() constructor [owner:api-engineer]
- [ ] [1.2] [P-1] Add list_emails(status: Option<String>) method — GET /emails, deserialize response, filter by status if provided, return Vec<ResendEmail> [owner:api-engineer]
- [ ] [1.3] [P-2] Add list_bounces() method — calls list_emails with status="bounced", returns Vec<ResendEmail> [owner:api-engineer]
- [ ] [1.4] [P-2] Add format_emails(emails: &[ResendEmail]) helper — formats as readable text (to, subject, status, timestamp) [owner:api-engineer]
- [ ] [1.5] [P-3] Add mod resend declaration in main.rs [owner:api-engineer]

## Tool Integration

- [ ] [2.1] [P-1] Register resend_emails tool in register_tools() — input schema: { status?: string } [owner:api-engineer]
- [ ] [2.2] [P-1] Register resend_bounces tool in register_tools() — input schema: {} (no params) [owner:api-engineer]
- [ ] [2.3] [P-2] Add dispatch cases in execute_tool() for both tools — call ResendClient methods, format output [owner:api-engineer]
- [ ] [2.4] [P-2] Init ResendClient in main.rs from RESEND_API_KEY env var — graceful fallback if missing [owner:api-engineer]

## Verify

- [ ] [3.1] cargo build passes [owner:api-engineer]
- [ ] [3.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [3.3] cargo test — new ResendClient tests (mock HTTP responses with wiremock) + existing tests pass [owner:api-engineer]
- [ ] [3.4] [user] Manual test: ask Nova "Any email bounces?" via Telegram, verify formatted response [owner:api-engineer]
