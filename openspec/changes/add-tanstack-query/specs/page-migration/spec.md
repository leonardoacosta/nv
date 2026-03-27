# Spec: Page Migration

## MODIFIED Requirements

### Requirement: Dashboard Data Fetching Pattern
All 18 dashboard pages MUST use `useApiQuery` for data fetching instead of raw `fetch` + `useEffect` + `useState`. Write operations MUST use `useApiMutation` with query invalidation on success.

#### Scenario: Home page parallel queries
Given the home page loads
When data fetching begins
Then 6 independent `useApiQuery` calls execute in parallel (activity-feed, obligations, messages, briefing, fleet-status, sessions)
And each query independently shows loading/error/data states
And auto-refresh uses `refetchInterval: 10_000` instead of `setInterval`

#### Scenario: Sessions page with filters
Given the sessions page renders with URL filter params
When the user changes project or trigger type filters
Then the query key updates to include the new filter params
And a new query executes automatically (cache miss)
And previous filter results remain cached for instant back-navigation

#### Scenario: Obligation creation with invalidation
Given the user creates an obligation from the home page quick-add bar
When the mutation succeeds
Then `["api", "/api/obligations"]` and `["api", "/api/activity-feed"]` queries are invalidated
And both lists refetch automatically

#### Scenario: Existing behavior preservation
Given any page is migrated from raw fetch to useApiQuery
When the page renders
Then WebSocket event integration continues to function via DaemonEventContext
And URL search param state is preserved for pages with filters
And all existing loading skeletons and empty states are replaced with the standardized components

### Requirement: useApiQuery Hook
A custom hook wrapping `useQuery` MUST integrate with the existing `apiFetch()` function for bearer token injection and 401 redirect handling. It SHALL accept a path string and optional configuration (params, enabled, refetchInterval, select, staleTime).

#### Scenario: Authenticated query
Given a user is logged in with a dashboard_token cookie
When `useApiQuery("/api/obligations")` executes
Then the request includes `Authorization: Bearer <token>` header

#### Scenario: 401 redirect
Given a user's session has expired
When any `useApiQuery` call receives a 401 response
Then the cookie is cleared and the user is redirected to /login

### Requirement: useApiMutation Hook
A custom hook wrapping `useMutation` MUST integrate with `apiFetch()` for authenticated write operations. It SHALL accept a path, HTTP method, and standard mutation options (onSuccess, onError).

#### Scenario: Successful mutation with invalidation
Given a mutation is configured with `onSuccess` that invalidates related queries
When the API call succeeds
Then `onSuccess` fires and the specified query keys are invalidated
And affected queries refetch in the background
