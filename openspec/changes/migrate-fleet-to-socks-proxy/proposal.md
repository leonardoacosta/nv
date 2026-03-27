# Proposal: Migrate Fleet from SSH+PowerShell to SOCKS Proxy

## Change ID
`migrate-fleet-to-socks-proxy`

## Summary

Replace SSH+PowerShell calls in graph-svc and teams-svc with direct HTTP through a SOCKS5 proxy
at localhost:1080. All Graph API and ADO calls become sub-second (0.4-0.6s) instead of 5-10s via
SSH+PowerShell. Uses `curl --socks5-hostname` via child_process -- no npm dependency needed.

## Context
- Modifies: `packages/tools/graph-svc/src/tools/calendar.ts`, `mail.ts`, `ado.ts`,
  `ado-extended.ts`, `pim.ts`
- Modifies: `packages/tools/teams-svc/src/tools/*.ts`
- New: `packages/tools/graph-svc/src/socks-client.ts` (shared SOCKS HTTP client)
- New: `packages/tools/graph-svc/src/token-cache.ts` (O365 + ADO token cache)
- New: `packages/tools/teams-svc/src/socks-client.ts` (shared SOCKS HTTP client for teams)
- New: `packages/tools/teams-svc/src/token-cache.ts` (O365 token cache for teams)
- Preserves: `ssh.ts` in both services (fallback when SOCKS unavailable)
- Existing: `ado-rest.ts` already uses native `fetch()` for ADO REST calls -- this change
  extends the same pattern to Calendar, Mail, Teams, PIM, and the legacy ADO tools

## Motivation

Proven benchmarks:
| Domain | SSH+PowerShell | SOCKS Proxy | Speedup |
|--------|---------------|-------------|---------|
| Calendar | 5.3s | 0.49s | 10.8x |
| Teams | 6.5s | 0.41s | 15.9x |
| Mail | 5.9s | 0.41s | 14.4x |
| ADO | 10s | 0.55s | 18.2x |

The SSH+PowerShell path adds ~5-10s of overhead per call: SSH handshake, PowerShell startup,
script parsing, text output formatting. The SOCKS proxy routes HTTP through the CloudPC tunnel
directly, returning raw JSON in 0.4-0.6s.

## Architecture

**Before:** `graph-svc -> SSH to CloudPC -> PowerShell script -> Graph/ADO API -> parse text`
**After:** `graph-svc -> curl --socks5-hostname localhost:1080 -> CloudPC -> Graph/ADO API -> JSON`

## Token Management

- **O365 token:** Cached in `.graph-token.json` on CloudPC, auto-refreshed every 30min by daemon
  cron. Read via one SSH call on first use, then cached in-memory with TTL.
- **ADO token:** Acquired via `az account get-access-token` on CloudPC. Read via SSH, cached
  in-memory. (ado-rest.ts already has this pattern.)
- **Token refresh:** Triggered on 401 response or when TTL expires. Proactive refresh by daemon
  cron for O365.

## Fallback Strategy

If SOCKS proxy is down (curl fails to connect to localhost:1080), fall back to existing SSH+PS
pattern. Proxy availability is checked on first call and cached with periodic re-check.

## Requirements

### Req-1: Shared SOCKS HTTP Client
Create `socks-client.ts` in both graph-svc and teams-svc. Uses `curl --socks5-hostname
localhost:1080` via `execFile` for GET/POST/PATCH requests. Returns raw response body. Throws
typed errors on failure. Supports configurable timeout.

### Req-2: Token Cache
Create `token-cache.ts` for O365 and ADO tokens. Tokens cached in memory with expiry tracking.
On first call or expiry: SSH to CloudPC to acquire token. O365 token read from
`.graph-token.json`. ADO token from `az account get-access-token`. Supports force-refresh on 401.

### Req-3: Calendar Tools via SOCKS
Rewrite `calendar.ts` to call Graph API directly: `/me/calendarView` with date range params.
Parse JSON response, format as text matching current PowerShell output format. Fall back to SSH
on SOCKS failure.

### Req-4: Mail Tools via SOCKS
Rewrite `mail.ts` to call Graph API directly: `/me/mailFolders/Inbox/messages`,
`/me/messages/{id}`, `/me/messages?$search=`, etc. Parse JSON, format text output. Fall back to
SSH.

### Req-5: ADO Tools via SOCKS
Rewrite `ado.ts` and `ado-extended.ts` to use `socksGet`/`socksPost` with ADO REST API URLs
directly. Token from existing `ado-rest.ts` cache. Fall back to SSH.

### Req-6: Teams Tools via SOCKS
Rewrite all `teams-svc/src/tools/*.ts` to call Graph API directly via SOCKS. List chats, read
messages, channels, presence, send. Fall back to SSH.

### Req-7: PIM Tools via SOCKS
Rewrite `pim.ts` to call Azure PIM REST API directly via SOCKS. Fall back to SSH.

### Req-8: Preserve SSH Fallback
Keep `ssh.ts` in both services. Add proxy availability detection. Each tool checks SOCKS first,
falls back to SSH on connection failure.

## Risks

- **SOCKS proxy down:** Mitigated by SSH fallback on every tool.
- **Token format change:** O365 `.graph-token.json` format is stable (daemon cron manages it).
- **Output format change:** Each tool must format JSON to match current PowerShell text output
  so Telegram command handlers continue working.
