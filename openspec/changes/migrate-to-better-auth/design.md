# Design: Migrate to Better Auth

## Architecture

```
packages/auth/               <-- NEW: @nova/auth
  src/
    index.ts                 <-- betterAuth() server config
    client.ts                <-- createAuthClient() for React
    seed.ts                  <-- Admin user + API key provisioning
  package.json

packages/db/
  src/schema/
    auth.ts                  <-- NEW: user, session, account, verification, apikey tables
    sessions.ts              <-- UNCHANGED: existing CC session tracking (plural "sessions")
  src/client.ts              <-- MODIFIED: register auth schema
  src/index.ts               <-- MODIFIED: export auth types
  drizzle/                   <-- NEW migration SQL

apps/dashboard/
  middleware.ts              <-- REWRITE: getSessionCookie() + legacy fallback
  app/api/auth/
    [...all]/route.ts        <-- NEW: Better Auth catch-all
    verify/route.ts          <-- REMOVE
    logout/route.ts          <-- REMOVE
  app/login/page.tsx         <-- REWRITE: email/password form
  lib/auth.ts                <-- REWRITE: re-export from @nova/auth
  lib/api-client.ts          <-- SIMPLIFY: remove Bearer injection
  components/providers/
    DaemonEventContext.tsx    <-- MODIFY: remove ?token= from WS URL
  server.ts                  <-- MODIFY: session cookie WS auth
  components/Sidebar.tsx     <-- MODIFY: signOut() instead of fetch
```

## Key Decisions

### Table Naming: Collision Avoidance

The existing `sessions` table (plural) tracks CC daemon sessions. Better Auth's default `session`
table (singular) is distinct in PostgreSQL. The Drizzle schema uses different variable names:
- `sessions` (existing) -> table `"sessions"`
- `authSession` (new) -> table `"session"`

If this causes confusion, Better Auth's adapter supports `modelName` overrides to use
`auth_session`, `auth_user`, etc. Prefer the default naming unless runtime issues arise.

### Middleware Strategy: Optimistic Cookie Check

Next.js 15.1.x middleware runs in Edge Runtime (no database access). The strategy:

1. **Middleware** (`getSessionCookie()`): checks cookie exists -> redirect to /login if absent
2. **Route handlers** (`auth.api.getSession()`): full DB session validation
3. **Server components**: can also call `auth.api.getSession()` for SSR auth

This is the pattern recommended by Better Auth docs for Next.js < 15.2.

### Migration Period: Dual Auth

During migration, the middleware accepts both:
1. Better Auth session cookie (checked via `getSessionCookie()`)
2. Legacy `DASHBOARD_TOKEN` cookie (checked via `timingSafeCompare()`)

API routes accept:
1. Better Auth session cookie (automatic, same-origin)
2. Better Auth API key (`Authorization: Bearer <api-key>`)
3. Legacy `DASHBOARD_TOKEN` bearer token (if `DASHBOARD_TOKEN` env var still set)

Removal of legacy auth: delete `DASHBOARD_TOKEN` from Doppler and deploy.

### Package Boundary: Why `packages/auth/` Not `apps/dashboard/lib/`

Better Auth config needs to be importable by both the dashboard app and potentially by the daemon
(for API key generation/validation). A workspace package keeps the auth config as a shared
dependency. The dashboard imports `@nova/auth` for the server instance and `@nova/auth/client`
for the React client.

### Session Cookie: httpOnly vs Accessible

Better Auth sets httpOnly session cookies by default. This is more secure than the previous
`dashboard_token` cookie (which was accessible to JavaScript for WebSocket auth). With Better Auth,
WebSocket auth uses the cookie sent automatically on the upgrade request -- no JavaScript access
needed.

### API Key for Service Access

The `@better-auth/api-key` plugin creates API keys tied to user accounts. The seed script creates
an admin user and generates an API key. This key is stored in Doppler as `NOVA_DASHBOARD_API_KEY`
for any future daemon-to-dashboard communication. The `bearer()` plugin resolves API keys from
`Authorization: Bearer` headers automatically.

## Environment Variables

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `BETTER_AUTH_SECRET` | Yes (prod) | None | Session encryption secret (32+ chars) |
| `BETTER_AUTH_URL` | Yes (prod) | None | Dashboard base URL (e.g. `https://nova.example.com`) |
| `NOVA_ADMIN_EMAIL` | No | `leo@nova.local` | Initial admin email (seed script) |
| `NOVA_ADMIN_PASSWORD` | Yes (seed) | None | Initial admin password (seed script) |
| `DASHBOARD_TOKEN` | No | None | Legacy auth (remove after migration) |
| `NOVA_DASHBOARD_API_KEY` | No | None | Generated API key (stored in Doppler after seed) |

## Dependencies Added

| Package | Where | Version | Purpose |
|---------|-------|---------|---------|
| `better-auth` | `packages/auth` | latest | Core auth framework |
| `@better-auth/api-key` | `packages/auth` | latest | API key plugin |
| `@nova/auth` | `apps/dashboard` | workspace:* | Auth package dependency |
