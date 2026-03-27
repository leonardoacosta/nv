# Proposal: Migrate to Better Auth

## Change ID
`migrate-to-better-auth`

## Depends On
`add-dashboard-auth` (existing bearer token auth -- this change replaces it)

## Summary

Replace the custom `DASHBOARD_TOKEN` bearer token auth with Better Auth, a TypeScript-first
authentication framework. This adds proper session management, password hashing, and user
accounts while preserving daemon-to-dashboard API access via Better Auth's API key plugin.

## Context
- Extends: `apps/dashboard/middleware.ts`, `apps/dashboard/lib/auth.ts`, `apps/dashboard/lib/api-client.ts`, `apps/dashboard/app/login/page.tsx`
- Replaces: `add-dashboard-auth` implementation (bearer token pattern)
- Related: `add-dashboard-auth` spec (75% complete -- all code tasks done, manual verification pending)
- Schema: `packages/db/src/schema/` (12 existing tables, Drizzle ORM + PostgreSQL)
- Database: `@nova/db` workspace package, `DATABASE_URL` env var, `drizzle-kit` for migrations

## Motivation

The current `DASHBOARD_TOKEN` auth (from `add-dashboard-auth`) is a single shared secret with no
sessions, no user accounts, and no password hashing. While functional for a homelab, it has
limitations:

1. **No session management** -- the raw token is stored in a cookie and sent on every request;
   there's no session ID, no expiry rotation, no revocation
2. **No user identity** -- all access uses the same token; no audit trail of who did what
3. **Token in URL** -- WebSocket auth passes the raw token as a query parameter
4. **No migration path to multi-user** -- adding a second user requires a fundamentally different
   auth system
5. **Manual token management** -- token is set via env var, not managed through a proper auth flow

Better Auth provides session-based auth with Drizzle integration, credential (email/password)
login, cookie-based session management, and an API key plugin for service-to-service access -- all
with minimal configuration.

## Requirements

### Req-1: Create `packages/auth/` Workspace Package

Create a new `@nova/auth` workspace package containing the Better Auth server configuration and
client helpers. The package exports the `auth` instance (server), an `authClient` (browser), and
TypeScript types inferred from the auth config. Uses the Drizzle adapter with `@nova/db`'s existing
`db` instance and `pg` provider. Credential auth (email/password) enabled. Bearer plugin enabled
for backward-compatible API access during migration. API key plugin enabled for daemon service
tokens.

### Req-2: Auth Schema Tables in Drizzle

Add Better Auth's required schema tables (`user`, `session`, `account`, `verification`) to
`packages/db/src/schema/`. Use Better Auth's CLI (`npx @better-auth/cli generate`) to generate
the Drizzle schema, then integrate it into the existing schema export pattern (individual files,
re-exported from `index.ts`, registered in `client.ts`). Generate a Drizzle migration via
`drizzle-kit generate`.

### Req-3: Auth API Route Handler

Create `apps/dashboard/app/api/auth/[...all]/route.ts` as the Better Auth catch-all handler using
`toNextJsHandler(auth)`. This replaces the existing `/api/auth/verify` and `/api/auth/logout`
routes. All Better Auth endpoints (sign-in, sign-up, sign-out, session, etc.) are served from
`/api/auth/*`.

### Req-4: Replace Middleware with Better Auth Session Check

Replace the custom bearer token middleware in `apps/dashboard/middleware.ts` with Better Auth's
`getSessionCookie()` check. Since Next.js 15.1.x uses Edge Runtime for middleware (no DB access),
the middleware performs cookie-presence validation only (optimistic redirect). Full session
validation happens in server components and API route handlers. API routes check session via
`auth.api.getSession({ headers })`. The middleware must still allow `/login`, `/api/auth/*`, and
static assets through without auth.

### Req-5: Replace Login Page with Email/Password Form

Replace the token-entry login page at `apps/dashboard/app/login/page.tsx` with an email/password
sign-in form. Uses Better Auth's `authClient.signIn.email()` for sign-in and
`authClient.signUp.email()` for initial account creation. Preserve the existing Nova branding and
dark theme styling. Add a toggle between sign-in and sign-up modes (sign-up only needed for
initial setup, can be disabled later via config).

### Req-6: Replace `apiFetch` with Session-Based Auth

Remove the `Authorization: Bearer` header injection from `apps/dashboard/lib/api-client.ts`.
Better Auth uses httpOnly session cookies that are automatically sent with `fetch` requests
(same-origin). The `apiFetch` wrapper simplifies to just handling 401 redirects (session expired).
All existing `apiFetch(...)` call sites remain unchanged -- only the implementation changes.

### Req-7: Replace WebSocket Auth with Session Cookie

Remove the `?token=` query parameter from WebSocket connections. Since Better Auth session cookies
are httpOnly and sent automatically on the upgrade request, `server.ts` validates the session
cookie on WebSocket upgrade instead. Uses `auth.api.getSession()` with the upgrade request headers.
Falls back to API key query parameter for non-browser clients.

### Req-8: Daemon Service Access via API Key

Configure Better Auth's API key plugin to support daemon-to-dashboard API calls (if needed in
future). Create a seed script that provisions an initial admin user and generates an API key for
service access. The API key is stored in Doppler as `NOVA_DASHBOARD_API_KEY`. API routes accept
either a valid session cookie OR an `Authorization: Bearer <api-key>` header (via the bearer
plugin).

### Req-9: Migration Path from Bearer Token

Provide a migration path from the existing `DASHBOARD_TOKEN` auth:

1. Deploy with Better Auth alongside existing auth (both patterns accepted temporarily)
2. Create initial admin user via seed script or sign-up page
3. Disable sign-up after initial user creation (optional config flag)
4. Remove `DASHBOARD_TOKEN` env var and legacy auth code
5. Remove `api-client.ts` Bearer header injection

The migration is non-breaking: existing cookie-based sessions continue to work during the
transition because the middleware falls back to checking the legacy cookie if no Better Auth
session exists.

## Scope
- **IN**: `packages/auth/` creation, Better Auth server + client config, Drizzle schema tables
  (user, session, account, verification), migration generation, catch-all auth route handler,
  middleware replacement, login page rewrite (email/password), `apiFetch` simplification, WebSocket
  session auth, API key plugin for service tokens, seed script, env vars (`BETTER_AUTH_SECRET`,
  `BETTER_AUTH_URL`)
- **OUT**: OAuth providers (Google, GitHub, etc.), magic links, passkeys, 2FA/MFA, RBAC/roles,
  organization/multi-tenant, rate limiting on login, email verification flow, password reset email,
  admin panel UI, multi-user invitation flow

## Impact
| Area | Change |
|------|--------|
| `packages/auth/` (new) | Better Auth server config, client, types |
| `packages/auth/package.json` (new) | `better-auth`, `@better-auth/api-key` deps |
| `packages/db/src/schema/auth.ts` (new) | user, session, account, verification tables |
| `packages/db/src/client.ts` | Register auth schema tables |
| `packages/db/src/index.ts` | Export auth schema + types |
| `packages/db/drizzle/` | New migration SQL |
| `apps/dashboard/package.json` | Add `@nova/auth` dep, `better-auth` deps |
| `apps/dashboard/middleware.ts` | Replace bearer check with `getSessionCookie()` |
| `apps/dashboard/app/api/auth/[...all]/route.ts` (new) | Better Auth catch-all handler |
| `apps/dashboard/app/api/auth/verify/route.ts` (remove) | Replaced by Better Auth |
| `apps/dashboard/app/api/auth/logout/route.ts` (remove) | Replaced by Better Auth |
| `apps/dashboard/app/login/page.tsx` | Rewrite: token input -> email/password form |
| `apps/dashboard/lib/auth.ts` | Rewrite: re-export from `@nova/auth` |
| `apps/dashboard/lib/api-client.ts` | Simplify: remove Bearer header injection |
| `apps/dashboard/components/providers/DaemonEventContext.tsx` | Remove `?token=` from WS URL |
| `apps/dashboard/server.ts` | Session cookie validation on WS upgrade |
| `apps/dashboard/components/Sidebar.tsx` | Update logout to use `authClient.signOut()` |
| `package.json` (root) | `packages/auth` in workspaces (already covered by glob) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Better Auth adds bundle size to dashboard | Use `better-auth/minimal` bundle; auth package is server-side only, client bundle is small |
| Drizzle adapter compatibility with existing `postgres` driver | Better Auth's Drizzle adapter accepts the `db` instance directly; same driver, no conflict |
| Next.js 15.1.x Edge middleware can't do DB session lookup | Cookie-presence check in middleware (optimistic); full validation in route handlers/server components |
| Schema naming collision (`sessions` table exists for CC sessions) | Better Auth tables use distinct names: `user`, `session` (singular), `account`, `verification` -- rename if needed to `auth_session` etc. via adapter config |
| WebSocket upgrade can't use httpOnly cookies in custom server.ts | Node.js `http.IncomingMessage` includes cookies; `auth.api.getSession()` works with raw headers |
| Migration downtime | Dual-auth period: accept both legacy cookie and Better Auth session during transition |
| API key plugin is a separate package (`@better-auth/api-key`) | Pin version alongside `better-auth` core; both are well-maintained |
