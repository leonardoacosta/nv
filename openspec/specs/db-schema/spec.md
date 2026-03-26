# db-schema Specification

## Purpose
TBD - created by archiving change setup-postgres-drizzle. Update Purpose after archive.
## Requirements
### Requirement: Shared Homelab Postgres Instance
Nova uses the shared homelab pgvector Postgres instance at port 5436 with a dedicated `nova` database. The project-local Docker Postgres service has been removed in favor of this shared instance.

#### Scenario: Shared instance connectivity
Given the shared Postgres instance is running on the homelab at port 5436,
Then the `nova` database is reachable at `localhost:5436` with credentials managed via Doppler,
And `DATABASE_URL` in Doppler (project: nova) points to the shared instance.

#### Scenario: pgvector extension available
Given the `nova` database exists on the shared instance,
Then `SELECT * FROM pg_available_extensions WHERE name = 'vector'` returns a row,
And `CREATE EXTENSION IF NOT EXISTS vector` succeeds without error.

### Requirement: Root TypeScript Workspace
A root `package.json` (private, workspaces: `apps/*`, `packages/*`) and `pnpm-workspace.yaml` SHALL be created so that `packages/db` is resolvable as `@nova/db` from any workspace package.

#### Scenario: pnpm workspace resolves packages
Given `pnpm install` is run at the project root,
Then `packages/db` is recognized as a workspace package,
And `apps/dashboard` can reference `@nova/db` without a relative path import.

### Requirement: packages/db Package Structure
A TypeScript package at `packages/db/` with name `@nova/db` SHALL be created including `package.json`, `tsconfig.json`, `drizzle.config.ts`, `src/client.ts`, and `src/index.ts`.

#### Scenario: Package builds without errors
Given `pnpm --filter @nova/db build` is run,
Then TypeScript compilation succeeds with zero errors,
And `dist/index.js` and `dist/index.d.ts` are emitted.

#### Scenario: DB client connects to local Postgres
Given `DATABASE_URL=postgres://nova:nova-local@localhost:5432/nova` is set and the postgres service is running,
Then `import { db } from '@nova/db'` resolves a live drizzle client,
And `db.select().from(messages).limit(1)` executes without error.

### Requirement: Messages Schema with pgvector
`packages/db/src/schema/messages.ts` SHALL define a `messages` table with columns: `id` (uuid pk), `channel` (text), `sender` (text, nullable), `content` (text), `metadata` (jsonb, nullable), `created_at` (timestamp), `embedding` (vector(1536), nullable) using a pgvector custom column type.

#### Scenario: messages table with vector column
Given the initial migration has been applied,
Then `\d messages` in psql shows all seven columns,
And the `embedding` column has type `vector(1536)`,
And an INSERT without an embedding value succeeds (embedding is nullable).

### Requirement: Obligations Schema
`packages/db/src/schema/obligations.ts` SHALL define an `obligations` table with columns: `id` (uuid pk), `detected_action` (text), `owner` (text), `status` (text), `priority` (integer), `project_code` (text, nullable), `source_channel` (text), `source_message` (text, nullable), `deadline` (timestamp, nullable), `last_attempt_at` (timestamp, nullable), `created_at` (timestamp), `updated_at` (timestamp).

#### Scenario: obligations table structure
Given the migration has been applied,
Then `\d obligations` shows all twelve columns,
And an INSERT with status `pending` succeeds.

### Requirement: Contacts Schema
`packages/db/src/schema/contacts.ts` SHALL define a `contacts` table with columns: `id` (uuid pk), `name` (text), `channel_ids` (jsonb), `relationship_type` (text, nullable), `notes` (text, nullable), `created_at` (timestamp).

#### Scenario: contacts table with jsonb channel_ids
Given the migration has been applied,
Then `\d contacts` shows all six columns,
And `channel_ids` column type is `jsonb`,
And an INSERT with `channel_ids = '{"telegram": "123456"}'::jsonb` succeeds.

### Requirement: Diary Schema
`packages/db/src/schema/diary.ts` SHALL define a `diary` table with columns: `id` (uuid pk), `trigger_type` (text), `trigger_source` (text), `channel` (text), `slug` (text), `content` (text), `tools_used` (jsonb, nullable), `tokens_in` (integer, nullable), `tokens_out` (integer, nullable), `response_latency_ms` (integer, nullable), `created_at` (timestamp).

#### Scenario: diary table structure
Given the migration has been applied,
Then `\d diary` shows all eleven columns,
And `tools_used` column type is `jsonb`.

### Requirement: Memory Schema
`packages/db/src/schema/memory.ts` SHALL define a `memory` table with columns: `id` (uuid pk), `topic` (text, unique), `content` (text), `updated_at` (timestamp).

#### Scenario: memory table with unique topic
Given the migration has been applied,
Then `\d memory` shows all four columns,
And inserting two rows with the same `topic` value raises a unique constraint violation.

### Requirement: Drizzle Config and Migration Scripts
`packages/db/drizzle.config.ts` SHALL point drizzle-kit at `src/schema/*.ts` with output dir `drizzle/`. The `package.json` scripts `db:generate`, `db:migrate`, and `db:studio` SHALL invoke the corresponding `drizzle-kit` commands.

#### Scenario: Migration generation
Given `pnpm --filter @nova/db db:generate` is run,
Then drizzle-kit reads all five schema files,
And produces SQL migration files in `packages/db/drizzle/`,
And the command exits 0.

#### Scenario: Apply migrations to local DB
Given the postgres service is running and `DATABASE_URL` is set,
When `pnpm --filter @nova/db db:migrate` is run,
Then all pending migrations are applied in order,
And the command exits 0.

