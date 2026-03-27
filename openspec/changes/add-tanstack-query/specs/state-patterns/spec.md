# Spec: Standardized State Patterns

## ADDED Requirements

### Requirement: QuerySkeleton Component
A reusable skeleton loading component MUST be available for all query-backed views. It SHALL accept `rows` (default 5) and `height` (default `"h-7"`) props and render pulse-animated placeholders using existing `bg-ds-gray-100` tokens.

#### Scenario: Default skeleton rendering
Given a query is in loading state
When QuerySkeleton renders with default props
Then 5 rows of h-7 height are displayed with pulse animation

#### Scenario: Custom skeleton dimensions
Given a page needs a card-style skeleton
When QuerySkeleton renders with `rows={3}` and `height="h-16"`
Then 3 rows of h-16 height are displayed

### Requirement: QueryErrorState Component
A reusable error state component MUST display the error message with a retry button. It SHALL accept `message: string` and optional `onRetry: () => void` props. Visual style MUST use `text-destructive` (red-700) and the existing ds-token system.

#### Scenario: Error with retry
Given a query has failed
When QueryErrorState renders with message "Failed to load" and an onRetry callback
Then the error message is displayed with an AlertCircle icon
And a "Try Again" button is visible and triggers onRetry when clicked

#### Scenario: Error without retry
Given a query has failed and no retry is possible
When QueryErrorState renders with message only
Then the error message is displayed without a retry button

### Requirement: QueryEmptyState Component
A reusable empty state component MUST display when a query returns an empty array. It SHALL accept optional `title`, `description`, and `onCreate` callback props.

#### Scenario: Empty with creation CTA
Given a query returns an empty array and an onCreate callback is provided
When QueryEmptyState renders
Then a title, description, and "Create" button are displayed

#### Scenario: Empty without CTA
Given a query returns an empty array and no onCreate is provided
When QueryEmptyState renders
Then only the title and description are displayed without a button

### Requirement: Canonical State Order
All query-consuming components MUST check states in order: Loading -> Error -> Empty -> Data. No component SHALL render data before checking for loading and error states.

#### Scenario: State ordering enforcement
Given a component uses `useApiQuery`
When the hook returns `{ data, isLoading, error }`
Then the component checks `isLoading` first, then `error`, then empty (`!data?.length`), then renders data
