# Implementation Tasks

<!-- beads:epic:TBD -->

## Auth Utility

- [x] [1.1] [P-1] Create apps/dashboard/lib/auth.ts -- export getToken(), isAuthEnabled(), verifyToken(candidate) using crypto.timingSafeEqual, AUTH_COOKIE_NAME = "dashboard_token", AUTH_COOKIE_MAX_AGE = 2592000 (30 days) [owner:ui-engineer]

## Middleware

- [x] [2.1] [P-1] Create apps/dashboard/middleware.ts -- NextRequest handler that gates all routes behind DASHBOARD_TOKEN bearer auth. Check cookie for page requests (redirect to /login if invalid), check Authorization header for /api/* requests (return 401 JSON), check token query param for /ws/* requests. Pass through if isAuthEnabled() returns false (dev mode). Use constant-time comparison via verifyToken() [owner:ui-engineer]
- [x] [2.2] [P-2] Add CORS handling to middleware -- read DASHBOARD_CORS_ORIGIN env var, set Access-Control-Allow-Origin/Methods/Headers on responses, handle OPTIONS preflight with 204. When DASHBOARD_CORS_ORIGIN is unset, omit the header (browser same-origin default) [owner:ui-engineer]
- [x] [2.3] [P-2] Add middleware matcher config -- exclude /_next/static, /_next/image, favicon.ico, /login from auth checks [owner:ui-engineer]

## Auth API Routes

- [x] [3.1] [P-1] Create apps/dashboard/app/api/auth/verify/route.ts -- POST handler accepting { token } JSON, compare via verifyToken(), return 200 { ok: true } or 401 { error: "Invalid token" } or 400 { error: "Token required" } [owner:ui-engineer]
- [x] [3.2] [P-2] Create apps/dashboard/app/api/auth/logout/route.ts -- POST handler that clears dashboard_token cookie (set max-age 0) and returns 200 { ok: true } [owner:ui-engineer]

## Login Page

- [x] [4.1] [P-1] Create apps/dashboard/app/login/page.tsx -- full-screen centered login form with NovaMark branding, single password input, submit calls POST /api/auth/verify, on success set dashboard_token cookie (sameSite: strict, path: /, max-age: 30 days) and redirect to /, on 401 show inline error. Dark theme matching bg-ds-bg-100, text-ds-text-100 [owner:ui-engineer]
- [x] [4.2] [P-2] Update apps/dashboard/app/layout.tsx -- conditionally exclude Sidebar and DaemonEventProvider wrapper for /login route (login page renders standalone without sidebar) [owner:ui-engineer]

## Fetch Wrapper

- [x] [5.1] [P-1] Create apps/dashboard/lib/api-client.ts -- export apiFetch(path, init?) that reads token from document.cookie, injects Authorization: Bearer header, calls fetch, and on 401 response clears cookie + redirects to /login [owner:ui-engineer]
- [x] [5.2] [P-1] Migrate all 44 client-side fetch("/api/...") and fetch(`/api/...`) calls across 21 files to use apiFetch(). Files: page.tsx, diary/page.tsx, integrations/page.tsx, messages/page.tsx, settings/page.tsx, sessions/page.tsx, sessions/[id]/page.tsx, nexus/page.tsx, projects/page.tsx, obligations/page.tsx, approvals/page.tsx, briefing/page.tsx, contacts/page.tsx, memory/page.tsx, usage/page.tsx, Sidebar.tsx, ActivityFeed.tsx, CCSessionPanel.tsx, ColdStartsPanel.tsx, LatencyChart.tsx, SessionDashboard.tsx, SessionWidget.tsx, UsageSparkline.tsx [owner:ui-engineer]

## WebSocket Auth

- [x] [6.1] [P-1] Update apps/dashboard/components/providers/DaemonEventContext.tsx -- read dashboard_token cookie value and append ?token=<value> to WebSocket URL in connect() [owner:ui-engineer]
- [x] [6.2] [P-2] Update apps/dashboard/server.ts -- validate token query param on WebSocket upgrade requests (req.url /ws/events) before proxying to daemon. Return 401 and destroy socket if invalid. Skip validation when DASHBOARD_TOKEN unset (dev mode) [owner:ui-engineer]

## Logout UI

- [x] [7.1] [P-2] Update apps/dashboard/components/Sidebar.tsx -- add "Log out" button at bottom of sidebar (below settings link). On click: POST /api/auth/logout, then redirect to /login. Use Lucide LogOut icon [owner:ui-engineer]

## Verify

- [x] [8.1] Grep audit -- confirm zero remaining bare fetch("/api/" or fetch(`/api/ calls (all migrated to apiFetch) [owner:ui-engineer]
- [ ] [8.2] Manual test -- start dashboard without DASHBOARD_TOKEN, verify all pages accessible (dev mode fallback) [owner:user]
- [ ] [8.3] Manual test -- start dashboard with DASHBOARD_TOKEN=test-token-12345678, verify unauthenticated page requests redirect to /login, unauthenticated API requests return 401 [owner:user]
- [ ] [8.4] Manual test -- enter token on login page, verify cookie set, redirected to /, all pages and API calls work with auth header [owner:user]
- [ ] [8.5] Manual test -- verify WebSocket connects with token query param, events stream normally [owner:user]
- [ ] [8.6] Manual test -- click logout, verify cookie cleared, redirected to /login [owner:user]
- [x] [8.7] npx tsc --noEmit -- dashboard compiles cleanly [owner:ui-engineer]
