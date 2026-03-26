# Implementation Tasks

<!-- beads:epic:nv-j7ih -->

## DB Batch

- [x] [1.1] [P-1] Create `nova` database on shared Postgres instance (port 5436) and enable pgvector extension [owner:db-engineer] [user] [beads:nv-9k3y]
- [x] [1.2] [P-1] Update `DATABASE_URL` in Doppler (project: nova, config: prd) to shared instance connection string [owner:db-engineer] [user] [beads:nv-gqjh]
- [x] [1.3] [P-1] Run `pnpm --filter @nova/db db:migrate` against shared instance to apply all existing migrations [owner:db-engineer] [beads:nv-yui6]
- [x] [1.4] [P-2] Verify all 6 tables exist on shared instance with correct column types including vector(1536) on messages.embedding [owner:db-engineer] [beads:nv-qx9f]

## API Batch

- [ ] [2.1] [P-1] Remove `postgres` service, `nova-pg-data` volume, and `docker/postgres-init` volume mount from `docker-compose.yml` [owner:api-engineer] [beads:nv-i0iz]
- [ ] [2.2] [P-2] Remove `docker/postgres-init/` directory (contains `01-enable-vector.sql`) [owner:api-engineer] [beads:nv-9i1e]
- [ ] [2.3] [P-2] Update `openspec/specs/db-schema/spec.md` Requirement "Docker Compose Postgres Service" scenarios to reference shared instance at port 5436 [owner:api-engineer] [beads:nv-9fo7]

## UI Batch

(No UI tasks -- this is an infrastructure-only change)

## E2E Batch

- [ ] [4.1] Restart `nova-ts.service` and verify daemon health endpoint responds at `/health` [owner:e2e-engineer] [beads:nv-s3b0]
- [ ] [4.2] Verify `docker compose up -d` succeeds with only the dashboard service (no postgres) [owner:e2e-engineer] [beads:nv-nvvv]
