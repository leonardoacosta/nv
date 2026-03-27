# Spec: auth-middleware

## MODIFIED Requirements

### Requirement: Middleware Session Check

The system SHALL replace the custom `DASHBOARD_TOKEN` bearer check in
`apps/dashboard/middleware.ts` with Better Auth's `getSessionCookie()` from `better-auth/cookies`.
The middleware MUST run in Edge Runtime (Next.js 15.1.x) and SHALL perform cookie-presence
validation only (no database lookups). It MUST redirect to `/login` if the session cookie is absent.

Routes excluded from auth check:
- `/login` (auth page)
- `/api/auth/*` (Better Auth endpoints)
- `/_next/static`, `/_next/image`, `favicon.ico` (static assets)

API routes (`/api/*` except `/api/auth/*`): return 401 JSON if no session cookie.
Page routes: redirect to `/login` if no session cookie.
CORS handling preserved from existing middleware.

#### Scenario: Authenticated page request passes through
Given a user has a valid Better Auth session cookie
When they request any page route
Then the middleware calls `NextResponse.next()`

#### Scenario: Unauthenticated page request redirects to login
Given no Better Auth session cookie is present
When a user requests a page route
Then the middleware redirects to `/login`

#### Scenario: Unauthenticated API request returns 401
Given no Better Auth session cookie or Bearer API key is present
When a request hits `/api/messages`
Then the middleware returns `{ "error": "Unauthorized" }` with status 401

#### Scenario: Auth endpoints pass through without check
Given any request to `/api/auth/sign-in/email`
When the middleware processes it
Then it passes through regardless of session state

#### Scenario: Dev mode fallback when BETTER_AUTH_SECRET unset
Given `BETTER_AUTH_SECRET` is not set in the environment
When any request arrives
Then the middleware passes through without auth checks (development mode)

### Requirement: Auth API Catch-All Route Handler

The system SHALL create `apps/dashboard/app/api/auth/[...all]/route.ts` using
`toNextJsHandler(auth)`. This single route MUST replace the previous `/api/auth/verify` and
`/api/auth/logout` routes.

#### Scenario: Sign-in endpoint is accessible
Given the catch-all route is deployed
When `POST /api/auth/sign-in/email` is called with `{ email, password }`
Then Better Auth processes the sign-in and sets a session cookie

#### Scenario: Sign-out endpoint clears session
Given a user is signed in
When `POST /api/auth/sign-out` is called
Then the session is revoked and the cookie is cleared

### Requirement: WebSocket Upgrade Auth

The system SHALL replace the `?token=` query parameter auth in `server.ts` with session cookie
validation. On WebSocket upgrade requests, it MUST extract cookies from the `req.headers.cookie`
header and validate the session using `auth.api.getSession({ headers })`. It SHALL reject the
upgrade with 401 if no valid session exists. The system MUST accept an API key in the
`Authorization` header as fallback for non-browser clients.

#### Scenario: Browser WebSocket upgrade with session cookie
Given a user is signed in with a valid session
When the browser opens a WebSocket to `/ws/events`
Then the session cookie is sent automatically and the upgrade succeeds

#### Scenario: WebSocket upgrade without auth is rejected
Given no session cookie or API key is present
When a WebSocket upgrade is attempted
Then the server returns 401 and destroys the socket
