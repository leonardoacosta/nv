# Spec: auth-package

## ADDED Requirements

### Requirement: Better Auth Server Configuration

The `@nova/auth` package SHALL export a configured `auth` instance using `betterAuth()` with:
- Drizzle adapter using `@nova/db`'s `db` instance and `pg` provider
- `emailAndPassword: { enabled: true }`
- `bearer()` plugin (for API key/token-based auth on API routes)
- `apiKey()` plugin from `@better-auth/api-key`
- `BETTER_AUTH_SECRET` env var (min 32 chars)
- `BETTER_AUTH_URL` env var (dashboard base URL)
- `basePath: "/api/auth"` (matches Next.js catch-all route)

#### Scenario: Auth instance initializes with Drizzle adapter
Given the `@nova/auth` package is imported
When `auth` is accessed
Then it is configured with the `@nova/db` database instance via `drizzleAdapter(db, { provider: "pg", schema })`

#### Scenario: Email/password auth is enabled
Given the auth instance is configured
When a client calls `POST /api/auth/sign-up/email`
Then a new user is created with hashed password

#### Scenario: API key plugin is active
Given the auth instance includes `apiKey()` plugin
When a request includes `Authorization: Bearer <api-key>` header
Then the request is authenticated via the API key

### Requirement: Better Auth Client Configuration

The `@nova/auth` package MUST export an `authClient` instance using `createAuthClient()` from
`better-auth/react` with matching plugin configuration. It SHALL export `signIn`, `signUp`,
`signOut`, `useSession` convenience methods and inferred types as `Session` and `User`.

#### Scenario: Client sign-in with email/password
Given the auth client is imported in a React component
When `signIn.email({ email, password })` is called
Then the user is authenticated and a session cookie is set

#### Scenario: Session hook provides user data
Given a user is signed in
When `useSession()` is called in a React component
Then it returns `{ data: { session, user }, isPending, error }`
