# Proposal: Migrate to Shared Homelab Postgres

## Change ID
`migrate-to-shared-postgres`

## Summary
Migrate Nova from its dedicated Docker Postgres container (port 5432) to the shared homelab pgvector Postgres instance (port 5436), consolidating with other project databases.

## Context
- Extends: `packages/db/src/client.ts`, `packages/db/drizzle.config.ts`, `docker-compose.yml`, `deploy/nova-ts.service`, `doppler.yaml`
- Related: Archived spec `setup-postgres-drizzle` (v9, established current Docker Postgres setup)
- Related: `openspec/specs/db-schema/spec.md` (existing requirements for Docker Compose Postgres)

## Motivation
Nova currently runs its own Docker Postgres container, duplicating infrastructure that already exists on the homelab. The shared instance at port 5436 runs `pgvector/pgvector:pg17`, supports the same extensions, and already hosts other project databases. Consolidating reduces resource usage, simplifies backups, and aligns with the homelab infrastructure strategy. This is a prerequisite for Wave 3 tool services (`add-memory-svc`, `add-messages-svc`) that depend on the shared Postgres pool.

## Requirements

### Req-1: Update DATABASE_URL to shared instance
The `DATABASE_URL` environment variable in Doppler (project: `nova`, config: `prd`) SHALL point to the shared homelab Postgres instance on port 5436 with a `nova` database.

### Req-2: Create nova database on shared instance
A `nova` database SHALL exist on the shared Postgres instance with the `vector` extension enabled, ready to receive Drizzle migrations.

### Req-3: Apply Drizzle migrations to shared instance
All existing Drizzle migrations SHALL be applied to the new `nova` database on the shared instance, producing identical schema to the current Docker Postgres.

### Req-4: Remove Docker Postgres service
The `postgres` service, `nova-pg-data` volume, and `docker/postgres-init/` init scripts SHALL be removed from `docker-compose.yml` since the dashboard container does not directly connect to Postgres (it hits the daemon API).

### Req-5: Update deploy scripts
The `deploy/pre-push.sh` script SHALL NOT reference or depend on the Docker Postgres service. The systemd service (`nova-ts.service`) already injects `DATABASE_URL` via Doppler, so no changes are needed there.

### Req-6: Update db-schema spec requirements
The `openspec/specs/db-schema/spec.md` requirement for "Docker Compose Postgres Service" SHALL be updated to reflect the shared instance rather than a project-local Docker container.

## Scope
- **IN**: Doppler secret update, database creation on shared instance, pgvector extension, Drizzle migration on shared instance, removal of Docker Postgres service/volume/init-scripts, spec update
- **OUT**: Data migration from old Docker container (fresh start -- no production data in Docker Postgres worth preserving), Rust daemon Postgres references (legacy, not active), dashboard Docker container changes (stays as-is, connects to daemon HTTP not Postgres)

## Impact
| Area | Change |
|------|--------|
| Infrastructure | Remove Docker Postgres container, use shared instance |
| Secrets | Update `DATABASE_URL` in Doppler nova/prd |
| Database | Create nova DB on shared instance, apply migrations |
| Deploy | Remove postgres references from docker-compose.yml |
| Specs | Update db-schema spec to reflect shared instance |

## Risks
| Risk | Mitigation |
|------|-----------|
| Shared instance unavailable | Shared instance is already proven with other projects; same pgvector image |
| Schema drift between old and new | Run `drizzle-kit migrate` to apply exact same migrations; verify with `drizzle-kit studio` |
| Port conflict or firewall | Shared instance already accessible on port 5436 by other services on the homelab |
| Dashboard loses DB access | Dashboard connects to daemon HTTP API, not directly to Postgres -- no impact |
