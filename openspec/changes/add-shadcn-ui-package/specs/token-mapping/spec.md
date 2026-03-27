# Spec: token-mapping

## ADDED Requirements

### Requirement: CSS variable bridge from ds-* to shadcn
The system SHALL map the existing `ds-*` CSS custom properties to shadcn's expected variable names so that shadcn components render with the Geist dark theme without changing any existing color values.

#### Scenario: Background and foreground mapping
Given `globals.css` defines `--ds-background-100: #0a0a0a` and `--ds-gray-1000: #ededed`
When the token bridge is added to `:root`
Then `--background` resolves to `0a0a0a` (HSL or raw, matching shadcn format)
And `--foreground` resolves to `ededed`
And existing `bg-ds-bg-100` and `text-ds-gray-1000` classes continue to work unchanged.

#### Scenario: Card and popover surface mapping
Given `surface-card` uses `--ds-gray-100` background and `--ds-gray-alpha-400` border
When shadcn `<Card>` renders
Then `--card` maps to the `ds-gray-100` value (`#1a1a1a`)
And `--card-foreground` maps to `ds-gray-1000` (`#ededed`)
And `--border` maps to `ds-gray-400` (`#2e2e2e`).

#### Scenario: Semantic color mapping
Given the project uses `red-700` for destructive, `green-700` for success, `amber-700` for warning, `blue-700` for accent
When the token bridge is added
Then `--destructive` maps to `#e5484d` (ds-red-700)
And `--primary` maps to `#ededed` (ds-gray-1000, matching the existing primary button style)
And `--muted` maps to `#1a1a1a` (ds-gray-100)
And `--muted-foreground` maps to `#a0a0a0` (ds-gray-900)
And `--accent` maps to the ds-gray-alpha-100 value
And `--ring` maps to `#454545` (ds-gray-500).

#### Scenario: Geist type scale and surface materials preserved
Given `globals.css` defines `.text-heading-32`, `.surface-card`, and 14 other custom utility classes
When the token bridge is added
Then all existing Geist type scale and surface material classes remain unchanged
And no shadcn CSS conflicts with or overrides these classes.
