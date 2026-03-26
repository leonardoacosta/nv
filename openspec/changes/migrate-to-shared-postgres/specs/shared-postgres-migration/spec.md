# shared-postgres-migration Specification

## Purpose
Migrate Nova's Postgres dependency from a project-local Docker container to the shared homelab pgvector instance, and update all configuration and specs accordingly.

## MODIFIED Requirements

### Requirement: Shared Postgres Connection
The `DATABASE_URL` environment variable SHALL point to the shared homelab Postgres instance at port 5436 with database name `nova`, replacing the Docker container at port 5432.

#### Scenario: Daemon connects to shared Postgres
Given Doppler project `nova` config `prd` has `DATABASE_URL` set to the shared instance connection string,
When `nova-ts.service` starts via `doppler run --project nova --config prd`,
Then the daemon's Drizzle client connects successfully,
And queries against the `messages` table return results without error.

#### Scenario: Drizzle config uses DATABASE_URL
Given `DATABASE_URL` is set to the shared instance connection string,
When `pnpm --filter @nova/db db:migrate` is run,
Then migrations apply to the shared instance's `nova` database,
And the command exits 0.

### Requirement: Nova Database on Shared Instance
A `nova` database SHALL exist on the shared Postgres instance with user credentials managed in Doppler, and the `vector` extension enabled.

#### Scenario: pgvector extension available on shared instance
Given the `nova` database exists on the shared instance,
When `SELECT * FROM pg_available_extensions WHERE name = 'vector'` is run,
Then a row is returned,
And `CREATE EXTENSION IF NOT EXISTS vector` succeeds.

#### Scenario: All existing tables created
Given Drizzle migrations have been applied to the `nova` database on the shared instance,
Then tables `messages`, `obligations`, `contacts`, `diary`, `memory`, and `briefings` all exist,
And the `messages.embedding` column has type `vector(1536)`.

## REMOVED Requirements

### Requirement: Docker Compose Postgres Service
The project-local `postgres` service in `docker-compose.yml` is REMOVED. The `nova-pg-data` volume and `docker/postgres-init/` init scripts are also removed. The shared homelab instance replaces this entirely.

#### Scenario: Docker Compose without Postgres
Given `docker compose config` is run on the updated `docker-compose.yml`,
Then no `postgres` service is defined,
And no `nova-pg-data` volume is defined,
And the `dashboard` service remains unchanged.
