# Spec: auth-ui

## MODIFIED Requirements

### Requirement: Email/Password Login Page

The system SHALL replace the token-entry form in `apps/dashboard/app/login/page.tsx` with an
email/password form using Better Auth's `authClient.signIn.email()`. It MUST preserve existing
Nova branding (NovaMark component), dark theme styling (`bg-ds-bg-100`, `text-ds-gray-1000`),
and centered layout.

The form MUST include email and password inputs. On submit, it SHALL call
`signIn.email({ email, password })`. On success, redirect to `/`. On error, show inline error.

The page SHALL include a toggle to switch between sign-in and sign-up modes. Sign-up MUST use
`signUp.email({ email, password, name })` with a name field.

#### Scenario: User signs in with email and password
Given the login page is displayed
When the user enters valid email/password and submits
Then `signIn.email()` is called, a session is created, and the user is redirected to `/`

#### Scenario: User sees error on invalid credentials
Given the login page is displayed
When the user enters wrong email/password and submits
Then an inline error "Invalid email or password" is shown

#### Scenario: User creates initial account via sign-up
Given the login page is in sign-up mode
When the user enters name, email, password and submits
Then `signUp.email()` creates a new user account and signs them in

### Requirement: Sidebar Logout

The logout button in `apps/dashboard/components/Sidebar.tsx` SHALL use `authClient.signOut()`
instead of calling `POST /api/auth/logout`. On success, it MUST redirect to `/login`.

#### Scenario: User clicks logout
Given the user is signed in and viewing the sidebar
When they click the "Log out" button
Then `signOut()` is called, the session is revoked, and the user is redirected to `/login`

### Requirement: Simplified API Client

The system SHALL simplify `apps/dashboard/lib/api-client.ts` by removing Bearer header injection.
Better Auth's httpOnly session cookies are sent automatically on same-origin requests. The
`apiFetch` wrapper MUST retain the 401 -> redirect-to-login behavior. All existing call sites
SHALL remain unchanged.

#### Scenario: API request includes session cookie automatically
Given a user is signed in with a Better Auth session
When `apiFetch("/api/messages")` is called
Then the session cookie is included automatically (browser default for same-origin)

#### Scenario: Expired session triggers login redirect
Given a user's session has expired
When `apiFetch("/api/obligations")` returns 401
Then the user is redirected to `/login`

### Requirement: WebSocket Connection Without Token Parameter

The system SHALL update `DaemonEventContext.tsx` to remove the `?token=` query parameter from the
WebSocket URL. The session cookie MUST be sent automatically on the WebSocket upgrade request.

#### Scenario: WebSocket connects without token in URL
Given a user is signed in
When the WebSocket connection is established
Then the URL is `/ws/events` (no `?token=` parameter)
And the session cookie authenticates the upgrade request
