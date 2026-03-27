# Spec: component-migration

## MODIFIED Requirements

### Requirement: Wave 1 leaf primitive migration
The system SHALL replace hand-built button, badge, input, select, skeleton, and separator patterns with shadcn equivalents from `@nova/ui`.

#### Scenario: Button replacement
Given `CreateProjectDialog` has an inline submit button with classes `bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900 rounded-lg`
When the button is replaced with `<Button>` from `@nova/ui`
Then the rendered output has identical visual appearance (same colors, padding, border-radius)
And the button gains consistent focus-visible ring behavior
And disabled state uses `opacity-50` matching the existing pattern.

#### Scenario: Badge replacement
Given `ContactCard` has inline badge spans with classes like `px-2 py-0.5 rounded-full text-label-12 font-medium`
When the badges are replaced with `<Badge variant="...">` from `@nova/ui`
Then the rendered output matches the existing pill appearance
And variants cover: default (neutral gray), destructive (red), success (green), warning (amber), outline (border-only).

#### Scenario: Input and Label replacement
Given `CreateProjectDialog` has 4 form fields with inline `<input>` and `<label>` elements
When replaced with shadcn `<Input>` and `<Label>` from `@nova/ui`
Then the input background, border, focus ring, placeholder color match the existing `bg-ds-gray-100 border-ds-gray-400 focus:border-ds-gray-1000/60` pattern
And the label matches `text-label-12 text-ds-gray-900` styling.

#### Scenario: Select replacement
Given `CreateProjectDialog` uses a native `<select>` for category
When replaced with shadcn `<Select>` from `@nova/ui`
Then a Radix-based dropdown renders with keyboard navigation (arrow keys, type-ahead)
And the trigger matches the existing select styling
And the dropdown menu uses `ds-bg-200` background with `ds-gray-400` border.

#### Scenario: Skeleton replacement
Given `PageSkeleton` uses `animate-shimmer` divs
When replaced with shadcn `<Skeleton>` from `@nova/ui`
Then the skeleton uses the same shimmer gradient (`ds-gray-alpha-200` to `ds-gray-alpha-400`)
And `PageSkeleton` internally composes `<Skeleton>` components instead of raw divs.

### Requirement: Wave 2 composed primitive migration
The system SHALL replace dialog, card, scroll area, and alert patterns with shadcn composed components from `@nova/ui`.

#### Scenario: Dialog replacement
Given `CreateProjectDialog` implements a custom modal with backdrop, escape handler, and manual focus management
When replaced with shadcn `<Dialog>` from `@nova/ui`
Then the dialog gains proper focus trapping (tab cycles within dialog)
And the backdrop uses the existing `bg-black/40 backdrop-blur-sm` styling
And the dialog content uses `bg-ds-bg-100 border-ds-gray-400 rounded-xl`
And escape key closes the dialog (Radix built-in, replacing the manual `useEffect` handler).

#### Scenario: Card replacement
Given `ActivityFeed` uses `surface-card` class on a container div
When `<Card>` from `@nova/ui` is available
Then components can optionally use `<Card>` for consistent structure (CardHeader, CardContent, CardFooter)
And the Card's default className applies the `surface-card` material
And existing `surface-card` usage on custom components remains valid (no forced migration).

#### Scenario: Alert replacement
Given `ErrorBanner` implements an inline error display with red background and retry button
When `<Alert variant="destructive">` from `@nova/ui` is available
Then `ErrorBanner` can be refactored to compose `<Alert>`, `<AlertTitle>`, `<AlertDescription>`
And the destructive variant uses `ds-red-700` for border and icon color
And the optional retry button is passed as a child.

#### Scenario: ScrollArea replacement
Given multiple components use `overflow-y-auto` with custom scrollbar CSS
When `<ScrollArea>` from `@nova/ui` is available
Then components can use `<ScrollArea>` for consistent cross-browser scrollbar appearance
And the scrollbar thumb uses `ds-gray-400` matching the existing `::-webkit-scrollbar-thumb` style.
