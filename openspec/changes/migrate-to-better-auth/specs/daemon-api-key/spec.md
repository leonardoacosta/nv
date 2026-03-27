# Spec: daemon-api-key

## ADDED Requirements

### Requirement: Admin User Seed Script

The system SHALL provide `packages/auth/src/seed.ts` -- a script that provisions the initial
admin user and generates an API key for service access. The script MUST:
1. Creates a user with email from `NOVA_ADMIN_EMAIL` env var (default: `leo@nova.local`)
2. Sets password from `NOVA_ADMIN_PASSWORD` env var
3. Generates an API key via Better Auth's API key plugin
4. Outputs the API key to stdout for Doppler storage

The script MUST be idempotent -- if the user already exists, it SHALL skip creation and only
generate a new API key if one doesn't exist.

#### Scenario: First run creates admin user and API key
Given no users exist in the database
When the seed script runs
Then an admin user is created and an API key is generated and printed

#### Scenario: Subsequent runs are idempotent
Given the admin user already exists
When the seed script runs again
Then it reports the user exists and does not create duplicates

### Requirement: API Key Authenticates Service Requests

API routes in the dashboard SHALL accept `Authorization: Bearer <api-key>` for service-to-service
calls. This MUST be handled by Better Auth's `bearer()` + `apiKey()` plugins -- any valid API key
in the Authorization header SHALL be resolved to a user session.

#### Scenario: Daemon calls dashboard API with API key
Given an API key exists for the admin user
When a request to `/api/messages` includes `Authorization: Bearer <api-key>`
Then the request is authenticated as the admin user

#### Scenario: Invalid API key is rejected
Given a request includes an invalid API key
When it hits any `/api/*` route
Then a 401 response is returned

### Requirement: Legacy Bearer Token Fallback (Migration Period)

During the migration period, the middleware and API routes SHALL accept BOTH:
1. Better Auth session cookies (new)
2. `DASHBOARD_TOKEN` bearer tokens (legacy, if env var still set)

Once migration is complete, the `DASHBOARD_TOKEN` fallback MUST be removed.

#### Scenario: Legacy token works during migration
Given `DASHBOARD_TOKEN` env var is still set
And Better Auth is deployed
When a request includes `Authorization: Bearer <DASHBOARD_TOKEN>`
Then the request is allowed (legacy fallback)

#### Scenario: Legacy fallback removed after migration
Given `DASHBOARD_TOKEN` env var is unset
When a request includes `Authorization: Bearer <old-token>`
Then the request is rejected with 401 (only Better Auth sessions and API keys accepted)
