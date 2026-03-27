# Spec: Query Provider

## ADDED Requirements

### Requirement: QueryClient Factory
The dashboard SHALL expose a `QueryClient` factory in `apps/dashboard/lib/query-client.ts` configured with 30-second stale time, 5-minute garbage collection, 3 retries with exponential backoff, and `refetchOnWindowFocus` enabled. Mutations MUST NOT auto-retry.

#### Scenario: Default query behavior
Given a page renders a `useApiQuery` hook
When the query data is less than 30 seconds old
Then no refetch is triggered on re-render

#### Scenario: Window focus refetch
Given a page has a stale query (older than 30 seconds)
When the user returns to the browser tab
Then the query automatically refetches in the background

#### Scenario: Retry on failure
Given an API request fails with a network error
When the query retries
Then it waits 1s, 2s, 4s between attempts (exponential backoff, max 10s)
And gives up after 3 retries

### Requirement: QueryClientProvider Integration
The `AppShell` component MUST wrap its children in `QueryClientProvider`. The login page MUST remain excluded (existing `isLoginPage` guard). React Query Devtools SHALL render only in development.

#### Scenario: Provider available in dashboard pages
Given a user navigates to any authenticated dashboard page
Then `useQueryClient()` is available in all child components

#### Scenario: Login page excluded
Given a user is on the login page
Then no `QueryClientProvider` is rendered
And no query-related code executes

#### Scenario: Devtools visibility
Given the app runs in development mode
Then the React Query Devtools panel is available at the bottom of the screen
And in production mode, devtools are not bundled
