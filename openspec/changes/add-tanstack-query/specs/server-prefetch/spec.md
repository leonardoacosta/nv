# Spec: Server-Side Prefetching

## ADDED Requirements

### Requirement: Server Prefetch for Home Page
The home page MUST prefetch critical data on the server using `dehydrate` and `HydrationBoundary` so the client renders from cache on first paint without showing a loading skeleton.

#### Scenario: First paint without loading spinner
Given a user navigates to the home page
When the server renders the page
Then activity-feed, obligations, and messages data are prefetched on the server
And the client hydrates from the dehydrated cache
And no loading skeleton is visible on initial render

#### Scenario: Server-side auth token access
Given the server needs to make authenticated API calls for prefetching
When the prefetch function executes
Then it reads the `dashboard_token` cookie via `next/headers`
And injects it as a Bearer token in the prefetch request

#### Scenario: Prefetch failure graceful degradation
Given the server-side prefetch fails (API unreachable)
When the client renders
Then the client falls back to normal client-side fetching with loading states
And no error is thrown during SSR

### Requirement: Server Prefetch for Briefing Page
The briefing page SHALL prefetch briefing data on the server for instant display on navigation.

#### Scenario: Briefing instant display
Given a user navigates to the briefing page
When the server renders the page
Then the briefing entry data is prefetched
And the client renders the briefing content without a loading state
