# Spec: auth-schema

## ADDED Requirements

### Requirement: Better Auth Schema Tables

The system SHALL add Better Auth's required tables to `packages/db/src/schema/auth.ts`:
- `user` -- id, name, email, emailVerified, image, createdAt, updatedAt
- `session` -- id, expiresAt, token, createdAt, updatedAt, ipAddress, userAgent, userId (FK -> user)
- `account` -- id, accountId, providerId, userId (FK -> user), accessToken, refreshToken, etc.
- `verification` -- id, identifier, value, expiresAt, createdAt, updatedAt

Table names must not collide with the existing `sessions` table (CC session tracking). Configure
Better Auth's adapter to use `auth_session` as the table name if collision occurs, or use Better
Auth's default singular naming (`session` vs existing `sessions`).

#### Scenario: Schema generated via Better Auth CLI
Given the Better Auth config exists at `packages/auth/src/index.ts`
When `npx @better-auth/cli generate` is run with `--output packages/db/src/schema/auth.ts`
Then a Drizzle schema file is created with all required auth tables

#### Scenario: Migration generated via drizzle-kit
Given the auth schema tables are added to `packages/db/src/schema/auth.ts`
When `pnpm db:generate` is run from the db package
Then a SQL migration is created in `packages/db/drizzle/`

#### Scenario: Schema registered in db client
Given `packages/db/src/schema/auth.ts` exports auth tables
When the schema is imported in `client.ts` and added to the schema object
Then `db.query.user`, `db.query.session`, `db.query.account`, `db.query.verification` are available

### Requirement: API Key Schema Table

The `apiKey()` plugin adds an `apikey` table. This MUST also be included in the generated schema
and migration.

#### Scenario: API key table exists after migration
Given the Better Auth config includes `apiKey()` plugin
When the schema is generated and migration is applied
Then an `apikey` table exists with columns: id, name, start, prefix, key, userId, refillInterval, refillAmount, lastRefillAt, enabled, rateLimitEnabled, rateLimitTimeWindow, rateLimitMax, requestCount, remaining, metadata, createdAt, updatedAt, expiresAt, permissions, deletedAt
