# Spec: Client Migration to tRPC

## MODIFIED Requirements

### Requirement: Migrate page components from apiFetch to tRPC
The system SHALL replace all `apiFetch()` calls in 18 page files and 11 component files with `useQuery(trpc.*.queryOptions())` and `useMutation(trpc.*.mutationOptions())`. Each page's `useState` + `useEffect` fetch pattern MUST be replaced by TanStack Query hooks consuming tRPC procedures.

#### Scenario: Home page migration
- GIVEN the home page currently uses 6 parallel `apiFetch` calls via `Promise.allSettled`
- WHEN migrated to tRPC
- THEN each fetch becomes an independent `useQuery(trpc.*.queryOptions())` call
- AND loading/error states are handled by TanStack Query instead of manual useState

#### Scenario: Obligations page CRUD migration
- GIVEN the obligations page uses apiFetch for GET, POST, PATCH
- WHEN migrated to tRPC
- THEN reads use `useQuery(trpc.obligation.list.queryOptions())`
- AND writes use `useMutation(trpc.obligation.create.mutationOptions({ onSuccess: ... }))`
- AND mutations invalidate `trpc.obligation.list.queryKey()` on success

#### Scenario: Component-level migration
- GIVEN components like `KanbanBoard`, `ActivityFeed`, `SessionWidget` use apiFetch
- WHEN migrated to tRPC
- THEN each component uses `useQuery(trpc.*.queryOptions())` with the appropriate procedure
- AND query invalidation uses typed query keys

### Requirement: Query invalidation via tRPC queryKey
The system SHALL replace all manual query key strings with `trpc.*.queryKey()` calls for type-safe invalidation.

#### Scenario: Mutation invalidates related queries
- GIVEN an obligation is created via `trpc.obligation.create`
- WHEN the mutation succeeds
- THEN `queryClient.invalidateQueries({ queryKey: trpc.obligation.list.queryKey() })` is called
- AND `queryClient.invalidateQueries({ queryKey: trpc.system.activityFeed.queryKey() })` is called

## REMOVED Requirements

### Requirement: Delete apiFetch and manual API types
After all client code is migrated to tRPC, the system SHALL delete the legacy API infrastructure.

#### Scenario: api-client.ts removal
- GIVEN no files import from `@/lib/api-client`
- WHEN the migration is complete
- THEN `apps/dashboard/lib/api-client.ts` is deleted

#### Scenario: types/api.ts removal
- GIVEN all types are inferred via `RouterOutputs` from `@nova/api`
- WHEN the migration is complete
- THEN `apps/dashboard/types/api.ts` (733 lines) is deleted
- AND any remaining type imports are replaced with `RouterOutputs["procedure"]["subprocedure"]`

#### Scenario: Route handler removal
- GIVEN all 49 route handlers are replaced by tRPC procedures
- WHEN the migration is complete
- THEN all files under `apps/dashboard/app/api/` are deleted except `api/trpc/[trpc]/route.ts`

#### Scenario: case.ts removal
- GIVEN `toSnakeCase` from `lib/case.ts` is no longer imported
- WHEN the migration is complete
- THEN `apps/dashboard/lib/case.ts` is deleted
