# Capability: Frontend Field Mapping Fixes

## MODIFIED Requirements

### Requirement: Obligations Page Response Unwrap and Field Mapping
`apps/dashboard/app/obligations/page.tsx` SHALL unwrap the daemon response `{ obligations: DaemonObligation[] }` and map daemon fields to the `ObligationItem` component's `Obligation` interface: `detected_action→title`, `source_message→description`, `deadline→due_at`, status `"done"→"completed"`, all other fields direct. The `tags` field is absent from the daemon response and MUST be omitted or set to `[]`.

#### Scenario: Obligations render with detected_action as title
Given the daemon returns `{ obligations: [{ id: "x1", detected_action: "Write tests for auth", status: "open", owner: "nova", priority: 1, created_at: "2026-03-25T10:00:00Z" }] }`, when ObligationsPage renders, then the ObligationItem row displays "Write tests for auth" as the visible title.

#### Scenario: Done status maps to history tab
Given an obligation with `status: "done"`, when mapped through the field transform, the resulting component prop is `status: "completed"`, and the obligation appears in the History tab rather than the Active tab.

### Requirement: Approvals Page Response Unwrap and Obligation-to-Approval Mapping
`apps/dashboard/app/approvals/page.tsx` SHALL unwrap `{ obligations: DaemonObligation[] }` from the `/api/obligations?owner=leo&status=open` response and map each daemon `Obligation` to the page's `Approval` interface: `detected_action→title`, `source_message→description`, `"open"→"pending"` and `"done"→"approved"` status translation, `priority→urgency` (0→"critical", 1→"high", 2→"medium", 3/4→"low"), `source_channel→project`, `action_type` defaults to `"other"`.

#### Scenario: Open leo obligation renders as pending approval
Given daemon returns `{ obligations: [{ id: "x1", detected_action: "Reply to Alice", priority: 2, status: "open", owner: "leo", source_channel: "telegram", created_at: "2026-03-25T10:00:00Z" }] }`, when ApprovalsPage renders, then one QueueItem shows title "Reply to Alice" and urgency badge "Medium".

#### Scenario: Empty obligations list shows empty state
Given `/api/obligations?owner=leo&status=open` returns `{ obligations: [] }`, when ApprovalsPage renders, then the EmptyState component with "No pending approvals" is displayed rather than a blank queue panel.

### Requirement: Projects Page Response Unwrap and ApiProject Mapping
`apps/dashboard/app/projects/page.tsx` SHALL unwrap `{ projects: ApiProject[] }` from the response (instead of casting directly as `Project[]`) and map each `ApiProject` to the `ProjectAccordion` `Project` interface: `code→id`, `code→name`, `path→path`, `status: "unknown"`, `errors: []`.

#### Scenario: Project list renders from daemon response
Given `{ projects: [{ code: "nv", path: "/home/nyaptor/nv" }] }`, when ProjectsPage renders, then one ProjectAccordion is visible with name "nv" and status badge "Unknown".

#### Scenario: Empty projects list shows empty state
Given `{ projects: [] }`, when ProjectsPage renders, then the "No projects found" empty state element is displayed — not a loading skeleton or blank space.

### Requirement: Integrations Page buildFromConfig Implementation
`apps/dashboard/app/integrations/page.tsx` SHALL implement the `buildFromConfig(raw: Record<string, unknown>): Integration[]` function that is called but missing. It MUST map known top-level config keys to `Integration` objects: telegram/discord/slack/teams → category "channels"; github/linear/notion → category "tools"; openai/anthropic/stripe/resend/sentry/posthog → category "services". Status is "connected" if the key's value is a non-empty object with at least one non-empty string entry, "disconnected" otherwise. Unknown keys are ignored.

Additionally, each category group in the render MUST show a "No integrations configured." placeholder when `items.length === 0` for that group.

#### Scenario: Telegram config key renders as connected card
Given config `{ "telegram": { "token": "abc123" } }`, when IntegrationsPage renders, then a Telegram integration card appears in the "Channels" section with status "Connected".

#### Scenario: Empty config renders placeholder in each section
Given config returns `{}`, when IntegrationsPage renders, then all three sections (Channels, Tools, Services) each display "No integrations configured." rather than invisible empty containers.

## ADDED Requirements

### Requirement: ObligationsGetResponse Type in api.ts
`apps/dashboard/types/api.ts` SHALL export an `ObligationsGetResponse` interface matching the daemon response: `{ obligations: DaemonObligation[] }` where `DaemonObligation` has fields `id`, `source_channel`, `source_message`, `detected_action`, `project_code`, `priority`, `status`, `owner`, `owner_reason`, `deadline`, `created_at`, `updated_at` — matching the Rust `Obligation` struct field names exactly.

#### Scenario: Typed cast catches field misuse at compile time
Given `ObligationsGetResponse` is imported in the obligations page and used as the cast type for `await res.json()`, when a developer attempts to access `data.obligations[0].title`, then TypeScript surfaces a compile-time error because the field is `detected_action`, not `title`.

### Requirement: Settings Page Empty Section Placeholder
`apps/dashboard/app/settings/page.tsx` SHALL render a "No fields configured." placeholder text inside each section card body when that section contains zero `FieldDef` entries. This ensures all four section cards (Daemon, Channels, Integrations, Memory) remain visually present and informative even when the daemon config is empty.

#### Scenario: Empty daemon config shows all four sections with placeholder
Given `GET /api/config` returns `{}`, when SettingsPage renders, then all four section cards are visible and each shows italic grey "No fields configured." text inside, not a blank content area.
