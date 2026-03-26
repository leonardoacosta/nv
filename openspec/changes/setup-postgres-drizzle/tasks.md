# Implementation Tasks

<!-- beads:epic:nv-ik8p -->

## Infra Batch

- [ ] [1.1] [P-1] Add postgres service to docker-compose.yml — image pgvector/pgvector:pg17, port 5432, volume nova-pg-data, credentials nova/nova-local/nova, networks homelab [owner:db-engineer]
- [ ] [1.2] [P-2] Add nova-pg-data named volume declaration to docker-compose.yml volumes section [owner:db-engineer]

## Workspace Batch

- [ ] [2.1] [P-1] Create root package.json — private:true, workspaces: ["apps/*", "packages/*"], no scripts required [owner:db-engineer]
- [ ] [2.2] [P-1] Create pnpm-workspace.yaml — packages: ["apps/*", "packages/*"] [owner:db-engineer]

## DB Package Batch

- [ ] [3.1] [P-1] Create packages/db/package.json — name @nova/db, scripts: db:generate, db:migrate, db:studio; deps: drizzle-orm, postgres; devDeps: drizzle-kit, typescript [owner:db-engineer]
- [ ] [3.2] [P-1] Create packages/db/tsconfig.json — strict, module ESNext, moduleResolution bundler, outDir dist, rootDir src [owner:db-engineer]
- [ ] [3.3] [P-1] Create packages/db/drizzle.config.ts — schema src/schema/*.ts, out drizzle/, driver pg, dbCredentials.url from DATABASE_URL env [owner:db-engineer]
- [ ] [3.4] [P-1] Create packages/db/src/client.ts — postgres.js-backed drizzle() client exported as db, reads DATABASE_URL [owner:db-engineer]
- [ ] [3.5] [P-1] Create packages/db/src/index.ts — re-exports db from client, all tables from schema/*.ts [owner:db-engineer]

## Schema Batch

- [ ] [4.1] [P-1] Create packages/db/src/schema/messages.ts — id uuid pk, channel text, sender text nullable, content text, metadata jsonb nullable, created_at timestamp, embedding vector(1536) custom type via pgvector [owner:db-engineer]
- [ ] [4.2] [P-1] Create packages/db/src/schema/obligations.ts — id uuid pk, detected_action text, owner text, status text, priority integer, project_code text nullable, source_channel text, source_message text nullable, deadline timestamp nullable, last_attempt_at timestamp nullable, created_at timestamp, updated_at timestamp [owner:db-engineer]
- [ ] [4.3] [P-1] Create packages/db/src/schema/contacts.ts — id uuid pk, name text, channel_ids jsonb, relationship_type text nullable, notes text nullable, created_at timestamp [owner:db-engineer]
- [ ] [4.4] [P-1] Create packages/db/src/schema/diary.ts — id uuid pk, trigger_type text, trigger_source text, channel text, slug text, content text, tools_used jsonb nullable, tokens_in integer nullable, tokens_out integer nullable, response_latency_ms integer nullable, created_at timestamp [owner:db-engineer]
- [ ] [4.5] [P-1] Create packages/db/src/schema/memory.ts — id uuid pk, topic text unique, content text, updated_at timestamp [owner:db-engineer]

## Migration Batch

- [ ] [5.1] [P-1] Run pnpm --filter @nova/db db:generate to produce initial migration in packages/db/drizzle/ and commit output [owner:db-engineer]

## Verify

- [ ] [6.1] pnpm --filter @nova/db build passes — zero TS errors [owner:db-engineer]
- [ ] [6.2] docker compose up -d postgres starts healthy [owner:db-engineer]
- [ ] [6.3] pnpm --filter @nova/db db:migrate applies initial migration against local DB [owner:db-engineer]
- [ ] [6.4] psql confirms all 5 tables exist with correct columns and embedding vector(1536) type [owner:db-engineer]
