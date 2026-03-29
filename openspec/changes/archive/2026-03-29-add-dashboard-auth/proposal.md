# Proposal: Add Dashboard Authentication

## Change ID
`add-dashboard-auth`

## Depends On
None (Wave 7, no dependencies)

## Summary

Add bearer token authentication to the Nova dashboard. Currently the dashboard binds `0.0.0.0`
with no auth, no CORS restrictions -- anyone who can reach the Traefik route can view and control
Nova. This change adds a `DASHBOARD_TOKEN` environment variable, a Next.js middleware that gates
all pages and API routes behind `Authorization: Bearer <token>`, a login page for token entry,
and CORS restrictions.

## Context
- Related idea: `nv-x3m` (dashboard-authentication)
- Dashboard: `apps/dashboard/` -- Next.js 15, React 19, Tailwind, Lucide
- Runs in Docker on homelab behind Traefik reverse proxy
- Custom `server.ts` proxies WebSocket `/ws/events` to daemon
- All client-side fetches use relative `/api/*` paths (26 API route handlers)
- No existing auth middleware, no auth library
- All API routes proxy to NV daemon via Next.js rewrites (`/api/:path*` -> `DAEMON_URL/api/:path*`)

## Motivation

Nova's dashboard exposes full read/write control over the daemon: session management, obligation
execution, config changes, solve triggers. Running unauthenticated behind a public Traefik route
means any network-adjacent actor could:

1. **Read sensitive data** -- messages, contacts, diary entries, memory, session logs
2. **Trigger actions** -- approve pending items, execute obligations, control sessions
3. **Modify config** -- change daemon behavior via settings page

A simple bearer token is the right level of auth for a single-user homelab dashboard. No user
management, no sessions, no OAuth -- just a shared secret that the browser remembers.

## Requirements

### Req-1: DASHBOARD_TOKEN Environment Variable

A new `DASHBOARD_TOKEN` env var (string, min 16 characters) is the shared secret. It must be set
in the Docker container via Doppler or direct env injection. If `DASHBOARD_TOKEN` is not set at
startup, the dashboard must still start but log a warning and operate without auth (development
mode fallback).

### Req-2: Next.js Middleware -- Token Gating

Create `apps/dashboard/middleware.ts` that:

1. Runs on all routes except `/login` and static assets (`_next/`, `favicon.ico`, etc.)
2. Checks `DASHBOARD_TOKEN` env var -- if unset, passes through (dev mode)
3. For page requests: checks for a `dashboard_token` cookie
   - If valid: passes through
   - If missing/invalid: redirects to `/login`
4. For API requests (`/api/*`): checks `Authorization: Bearer <token>` header
   - If valid: passes through
   - If missing/invalid: returns 401 JSON `{ "error": "Unauthorized" }`
5. For WebSocket upgrade requests (`/ws/*`): checks `token` query parameter
   - If valid: passes through
   - If missing/invalid: returns 401
6. Uses constant-time comparison (`timingSafeEqual`) for token validation

The middleware matcher config must exclude Next.js internals and static files.

### Req-3: Login Page

Create `apps/dashboard/app/login/page.tsx`:

1. Full-screen centered form with Nova branding (NovaMark component)
2. Single password input for the token
3. On submit: `POST /api/auth/verify` with `{ token }` body
4. On success (200): set `dashboard_token` cookie (httpOnly: false so JS can read for WS,
   sameSite: strict, path: /, max-age: 30 days), redirect to `/`
5. On failure (401): show inline error "Invalid token"
6. Styling: matches existing dashboard dark theme (`bg-ds-bg-100`, `text-ds-text-100`)
7. No "remember me" checkbox -- always persists for 30 days

### Req-4: Auth Verification API Route

Create `apps/dashboard/app/api/auth/verify/route.ts`:

1. `POST` handler accepts `{ token: string }` JSON body
2. Compares token against `DASHBOARD_TOKEN` using constant-time comparison
3. Returns 200 `{ ok: true }` on match
4. Returns 401 `{ error: "Invalid token" }` on mismatch
5. Returns 400 `{ error: "Token required" }` if body missing

### Req-5: Auth Utility Module

Create `apps/dashboard/lib/auth.ts`:

1. `getToken()` -- reads `DASHBOARD_TOKEN` from `process.env`
2. `isAuthEnabled()` -- returns `true` if `DASHBOARD_TOKEN` is set and non-empty
3. `verifyToken(candidate: string)` -- constant-time comparison against stored token
4. `AUTH_COOKIE_NAME` constant = `"dashboard_token"`
5. `AUTH_COOKIE_MAX_AGE` constant = `60 * 60 * 24 * 30` (30 days)

### Req-6: Fetch Wrapper with Authorization Header

Create `apps/dashboard/lib/api-client.ts`:

1. `apiFetch(path: string, init?: RequestInit)` -- wraps `fetch` with auth header injection
2. Reads token from cookie (`document.cookie` parse) on the client side
3. Injects `Authorization: Bearer <token>` header on all requests
4. If response is 401: clears cookie, redirects to `/login`
5. All existing `fetch("/api/...")` calls across components/pages must be migrated to use
   `apiFetch` instead

### Req-7: WebSocket Auth

Update `apps/dashboard/components/providers/DaemonEventContext.tsx`:

1. When constructing the WebSocket URL, append `?token=<value>` from cookie
2. Update `apps/dashboard/server.ts` to validate the `token` query parameter on WebSocket
   upgrade requests before proxying to daemon
3. On auth failure: reject the upgrade with 401

### Req-8: CORS Configuration

Add CORS headers in `apps/dashboard/middleware.ts`:

1. For all responses, set:
   - `Access-Control-Allow-Origin`: value of `DASHBOARD_CORS_ORIGIN` env var, or same-origin
     if unset
   - `Access-Control-Allow-Methods`: `GET, POST, PUT, DELETE, OPTIONS`
   - `Access-Control-Allow-Headers`: `Content-Type, Authorization`
2. Handle OPTIONS preflight requests with 204 response
3. `DASHBOARD_CORS_ORIGIN` is optional -- when unset, no `Access-Control-Allow-Origin` header
   is sent (browser enforces same-origin by default)

### Req-9: Logout

Add logout capability:

1. `apps/dashboard/app/api/auth/logout/route.ts`: POST handler that clears the
   `dashboard_token` cookie and returns 200
2. Add a "Log out" button to the sidebar (bottom, near settings) that calls the logout endpoint
   and redirects to `/login`

## Scope
- **IN**: Middleware, login page, auth utility, fetch wrapper migration, WebSocket auth, CORS,
  logout, Dockerfile env passthrough
- **OUT**: User management, role-based access, OAuth/OIDC, session tokens with expiry/refresh,
  rate limiting on login attempts, Doppler secret creation (manual step)

## Impact
| Area | Change |
|------|--------|
| `middleware.ts` (new) | Token gating for all routes + CORS headers |
| `app/login/page.tsx` (new) | Login form |
| `app/api/auth/verify/route.ts` (new) | Token verification endpoint |
| `app/api/auth/logout/route.ts` (new) | Cookie-clearing logout endpoint |
| `lib/auth.ts` (new) | Auth utility functions |
| `lib/api-client.ts` (new) | Fetch wrapper with auth header injection |
| `components/providers/DaemonEventContext.tsx` | Add token query param to WS URL |
| `server.ts` | Validate token on WebSocket upgrade |
| `components/Sidebar.tsx` | Add logout button |
| `app/layout.tsx` | Exclude login page from sidebar layout |
| 15 page/component files | Migrate `fetch("/api/...")` to `apiFetch(...)` |
| `Dockerfile` | No change needed (env vars pass through at runtime) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Token leaked in browser localStorage/cookie | httpOnly: false needed for WS, but sameSite: strict + HTTPS via Traefik limits exposure |
| Constant-time compare not available in Edge Runtime | Use `crypto.subtle` or polyfill `timingSafeEqual` for Edge middleware |
| Breaking existing dashboard access during rollout | Dev-mode fallback when DASHBOARD_TOKEN unset -- no auth enforced |
| Fetch wrapper migration misses a call site | Grep audit in tasks; TypeScript won't catch runtime-only auth failures |
| WebSocket token in URL visible in server logs | Token is short-lived in URL params only; Traefik access logs can be filtered |
