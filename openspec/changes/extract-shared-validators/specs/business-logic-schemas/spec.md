# Spec: Business-Logic DTO Schemas

## ADDED Requirements

### Requirement: Create DTO Schemas
The system SHALL provide create-input Zod schemas for entities with active write paths: messages, obligations, contacts, projects, memory, reminders, schedules, sessions, briefings, and settings. Each create schema MUST omit server-generated fields (id, createdAt, updatedAt) and apply business-rule constraints (e.g., required fields, string min-lengths).

#### Scenario: Create obligation schema enforces required fields
Given `createObligationSchema` is defined
When an object missing `detectedAction` is parsed
Then Zod throws a validation error indicating the field is required

#### Scenario: Create contact schema accepts valid input
Given `createContactSchema` requires `name` and `channelIds`
When `{ name: "Alice", channelIds: { telegram: "123" } }` is parsed
Then parsing succeeds and the result is typed as `CreateContactInput`

#### Scenario: Create project schema matches existing behavior
Given the existing `createProjectSchema` from `packages/db/src/schema/projects.ts`
When the same input is parsed through the new validators version
Then both schemas produce identical validation results

### Requirement: Update DTO Schemas
The system SHALL provide update-input Zod schemas for writable entities. Update schemas MUST make all fields optional via `.partial()` and exclude immutable fields (id, createdAt).

#### Scenario: Update obligation schema allows partial updates
Given `updateObligationSchema` is defined
When `{ status: "done" }` is parsed (only one field)
Then parsing succeeds with all other fields undefined

#### Scenario: Update schema rejects immutable fields
Given `updateContactSchema` is defined
When an object with `id: "some-uuid"` is parsed
Then the `id` field is stripped (not present in output) since it is not in the schema

### Requirement: Filter and Pagination Schemas
The system SHALL provide reusable filter schemas for list endpoints (status, owner, channel, date-range filters) and shared pagination schemas (limit, offset, cursor) with sensible defaults.

#### Scenario: Pagination schema applies defaults
Given `paginationSchema` with default `limit: 20` and `offset: 0`
When an empty object `{}` is parsed
Then the result is `{ limit: 20, offset: 0 }`

#### Scenario: Obligation filter schema validates status
Given `obligationFilterSchema` has a `status` field
When `{ status: "invalid_status" }` is parsed with an enum constraint
Then Zod throws a validation error for the invalid enum value

#### Scenario: Date range filter accepts ISO strings
Given `dateRangeSchema` has optional `from` and `to` fields
When `{ from: "2026-01-01T00:00:00Z" }` is parsed
Then parsing succeeds and `from` is coerced to a Date object
