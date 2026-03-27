# Proposal: Extract Shared Validators Package

## Change ID
`extract-shared-validators`

## Summary
Create a `@nova/validators` workspace package that generates Zod schemas from Drizzle tables using `drizzle-zod` and adds business-logic DTOs (create, update, filter, pagination), replacing scattered inline validation across tool services and unvalidated dashboard route handlers.

## Context
- Extends: `packages/db/src/schema/` (12 Drizzle table files), `packages/tools/*/src/mcp.ts` (inline Zod schemas), `apps/dashboard/app/api/*/route.ts` (unvalidated handlers)
- Related: `add-trpc-api` (empty, will consume validators for procedure inputs), `rewire-dashboard-api` (route handlers that lack validation)

## Motivation
Validation is scattered and inconsistent across the project. Tool services define inline `z.object()` schemas that duplicate column knowledge from Drizzle tables. Dashboard route handlers have zero Zod validation -- POST bodies are parsed with raw `request.json()` and manual `typeof` checks (e.g., obligations POST) or no checks at all (e.g., contacts POST). The `projects.ts` schema is the only file that co-locates Zod schemas with Drizzle, but this pattern was never extended to the other 11 tables. A centralized validators package eliminates duplication, provides type-safe DTOs for the upcoming tRPC migration, and ensures consistent input validation everywhere.

## Requirements

### Req-1: Workspace Package Scaffold
Create `packages/validators/` as a new `@nova/validators` workspace package with `drizzle-zod` and `zod` as dependencies, ESM output, and TypeScript strict mode matching existing packages.

### Req-2: Drizzle-Derived Base Schemas
Generate `createInsertSchema` and `createSelectSchema` for all 12 Drizzle tables using `drizzle-zod`. Each entity gets its own file mirroring `packages/db/src/schema/` structure.

### Req-3: Business-Logic DTO Schemas
Layer custom business-logic schemas on top of the Drizzle-derived base schemas: create DTOs (required fields, defaults stripped), update DTOs (all fields optional via `.partial()`), filter schemas (query parameter shapes), and shared pagination/sorting schemas.

### Req-4: Consumer Exports
Export all schemas and inferred TypeScript types from a barrel `index.ts` for consumption by: tRPC procedures (`add-trpc-api`), dashboard route handlers, tool services, and CLI. Migrate the existing `projects.ts` Zod schemas out of `packages/db/` into the validators package.

## Scope
- **IN**: Package scaffold, drizzle-zod generation for all 12 tables, create/update/filter DTOs for entities with write operations (messages, obligations, contacts, projects, memory, reminders, schedules, sessions, briefings, settings), pagination/sorting shared schemas, barrel exports, migration of projects.ts Zod schemas from db package
- **OUT**: Rewriting tool service MCP `inputSchema` declarations (tool schemas are MCP-specific with `.describe()` annotations -- consumers adopt validators incrementally), rewriting dashboard route handlers to use validators (covered by `rewire-dashboard-api` / `add-trpc-api`), runtime form validation in React components, E2E tests for validators (unit tests only)

## Impact
| Area | Change |
|------|--------|
| `packages/validators/` | New workspace package |
| `packages/db/src/schema/projects.ts` | Remove Zod schemas (moved to validators) |
| `packages/db/src/index.ts` | Remove Zod re-exports from projects |
| `packages/db/package.json` | Remove `zod` dependency (no longer needed in db) |
| `package.json` (root) | `packages/*` workspace glob already covers new package |

## Risks
| Risk | Mitigation |
|------|-----------|
| `drizzle-zod` version incompatibility with drizzle-orm 0.39.x | Pin compatible version; drizzle-zod 0.7.x supports 0.39.x |
| Breaking `projects.ts` Zod imports in existing consumers | Update import paths in db barrel; search for all `@nova/db` Zod imports |
| Schema drift between Drizzle tables and validators | Validators import Drizzle tables directly; `createInsertSchema` stays in sync by construction |
| Over-engineering DTOs for tables with no current write operations (diary, session-events) | Only create DTOs for entities with active write paths; read-only tables get select schemas only |
