# Spec: Drizzle-Zod Base Schemas

## ADDED Requirements

### Requirement: Validators Package Scaffold
The system SHALL provide a `@nova/validators` workspace package at `packages/validators/` with `zod` and `drizzle-zod` as dependencies, ESM module output, TypeScript strict mode, and `@nova/db` as a peer dependency.

#### Scenario: Package initializes and builds
Given the `packages/validators/` directory exists with `package.json`, `tsconfig.json`, and `src/index.ts`
When `pnpm build` is run from the package directory
Then TypeScript compilation succeeds with zero errors and `dist/` contains `.js` and `.d.ts` files

#### Scenario: Package is importable from other workspace packages
Given `@nova/validators` is listed as a dependency in a consuming package
When the consumer imports `{ createMessageSchema } from "@nova/validators"`
Then the import resolves correctly and TypeScript provides full type inference

### Requirement: Drizzle-Derived Insert Schemas
The system SHALL generate Zod insert schemas from all 12 Drizzle tables using `createInsertSchema` from `drizzle-zod`. Each entity file MUST mirror the `packages/db/src/schema/` naming.

#### Scenario: Insert schema matches table columns
Given the `messages` Drizzle table has columns `id`, `channel`, `sender`, `content`, `metadata`, `createdAt`, `embedding`
When `insertMessageSchema` is generated via `createInsertSchema(messages)`
Then the schema accepts objects matching the table's insert type with `id` and `createdAt` optional (defaulted columns)

#### Scenario: Insert schema rejects invalid data
Given `insertObligationSchema` requires `detectedAction` as a non-empty string
When an object with `detectedAction: ""` is parsed
Then Zod throws a validation error

### Requirement: Drizzle-Derived Select Schemas
The system SHALL generate Zod select schemas from all 12 Drizzle tables using `createSelectSchema` from `drizzle-zod` for validating data read from the database.

#### Scenario: Select schema validates database rows
Given a row returned from `db.select().from(obligations)`
When the row is parsed through `selectObligationSchema`
Then parsing succeeds and returns a typed `Obligation` object
