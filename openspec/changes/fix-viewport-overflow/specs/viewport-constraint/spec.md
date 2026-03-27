# Spec: viewport-constraint

## MODIFIED Requirements

### Requirement: AppShell viewport constraint
The AppShell root wrapper div MUST use exact viewport height (`h-dvh`) with hidden overflow instead of minimum viewport height (`min-h-dvh`), ensuring the dashboard is a fixed-viewport application.

#### Scenario: Dashboard page renders within viewport bounds
- **Given** a user navigates to any dashboard page
- **When** the page content exceeds the viewport height
- **Then** the outer wrapper does not grow beyond `100dvh`
- **And** only the main content area scrolls vertically
- **And** the sidebar remains fixed in position

#### Scenario: Login page bypasses AppShell
- **Given** the user is on the login page (`/login`)
- **When** the page renders
- **Then** AppShell returns children directly without the viewport-constrained wrapper
- **And** no layout constraint is applied
