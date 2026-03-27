# Spec: shared-ui-package

## ADDED Requirements

### Requirement: packages/ui workspace package
The system SHALL provide a `packages/ui/` pnpm workspace package named `@nova/ui` that initializes shadcn/ui with a Geist dark theme and exports components for consumption by `apps/dashboard`.

#### Scenario: Package initialization
Given the monorepo root `package.json` already declares `"workspaces": ["packages/*"]`
When `packages/ui/` is created with `package.json` declaring `"name": "@nova/ui"`
Then `pnpm install` resolves `@nova/ui` as a workspace dependency
And the package contains `components.json` for shadcn CLI
And `tailwind.config.ts` extends the root Geist token palette.

#### Scenario: Component export pattern
Given `packages/ui/src/components/button.tsx` exists
When `packages/ui/src/index.ts` re-exports `{ Button }` from `./components/button`
Then `apps/dashboard` can import `{ Button } from "@nova/ui"`
And tree-shaking eliminates unused components from the production bundle.

#### Scenario: Dashboard consumes @nova/ui
Given `apps/dashboard/package.json` declares `"@nova/ui": "workspace:*"`
When the dashboard build runs
Then Tailwind scans `packages/ui/src/**/*.tsx` for class names
And shadcn component styles render identically to the existing hand-built versions.
