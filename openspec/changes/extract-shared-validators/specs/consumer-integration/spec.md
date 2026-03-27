# Spec: Consumer Integration and Migration

## ADDED Requirements

### Requirement: Barrel Export Structure
The system SHALL export all schemas and inferred TypeScript types from `packages/validators/src/index.ts` organized by entity, allowing tree-shakeable imports like `import { createObligationSchema, type CreateObligationInput } from "@nova/validators"`.

#### Scenario: All entity schemas are importable
Given the validators package is built
When a consumer imports `{ insertMessageSchema, createObligationSchema, paginationSchema }` from `@nova/validators`
Then all imports resolve and TypeScript infers correct types

#### Scenario: Type inference works from schemas
Given `createContactSchema` is exported
When a consumer uses `z.infer<typeof createContactSchema>`
Then TypeScript infers the correct `CreateContactInput` type without explicit type annotation

### Requirement: Migrate Projects Zod Schemas
The system SHALL move the existing Zod schemas (`projectCategoryEnum`, `projectStatusEnum`, `createProjectSchema`, `updateProjectSchema`) and their inferred types from `packages/db/src/schema/projects.ts` into `packages/validators/src/projects.ts`, updating the `@nova/db` barrel to remove Zod re-exports and the `zod` dependency from the db package.

#### Scenario: Existing project schema consumers still compile
Given consumers import `createProjectSchema` from `@nova/db`
When the import is changed to `@nova/validators`
Then the schema has identical shape and validation behavior

#### Scenario: DB package no longer depends on Zod
Given `packages/db/package.json` previously listed `zod` in dependencies
When the migration is complete
Then `zod` is removed from `packages/db/package.json` and `pnpm build` succeeds

## MODIFIED Requirements

### Requirement: DB Package Export Surface (from db-schema spec)
The `@nova/db` barrel index SHALL export Drizzle table objects and Drizzle-inferred types (`$inferSelect`, `$inferInsert`) only. Zod schemas and Zod-inferred types MUST be exported from `@nova/validators` instead.

#### Scenario: DB barrel has no Zod exports
Given `packages/db/src/index.ts` is updated
When the file is inspected
Then it contains no imports from `zod` and no Zod schema exports
