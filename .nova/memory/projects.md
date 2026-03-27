# Project Registry

## Priority Stack (confirmed by Leo)
oo → ct → tl → mv → ws → nv

## Projects

### OO (Otaku Odyssey) — Convention Management
- **Stack**: T3 Turbo, Vercel, Stripe, Expo mobile, QR badges
- **Status**: Most mature, gold standard for other projects
- **Jira**: 15 epics defined (Auth, Registration, Vendors, Sponsors, Panelists, Ambassadors, Cosplay [Runway absorbed], Comms, Facilities, Scheduling, Staff Portal, Staff, Volunteers, Tech Debt, CI/CD)
- **Notes**: ~80 specs queued, batch of P2 specs from March 21 unexecuted. Existing ~40+ open epics need mapping/cleanup/retrofit.

### CT (Civilant) — RAG Compliance/Feasibility Tool
- **Stack**: Next.js 16, Claude compliance pipeline, hybrid RRF search
- **Origin**: Spun out of FP (Flughafen PM) — AI consulting for Dallas airport design firm
- **Dev**: James built 20+ regulatory collectors, substantial code in `apps/civalent/`
- **Framework**: AI-assisted + human review gating (NOT fully agentic)
- **Jira**: 10 epics defined, but E1 (Regulatory Corpus) + E2 (AI Compliance Engine) never landed (pagination bug). CT needs separate LLC Jira instance.
- **Issues**: `@fp/*` → `@ct/*` rename not done, `gpt-5.4` typo in chat route, Beads not initialized

### TL (Tavern Ledger) — D&D Campaign Manager
- **Jira**: 6 epics. P1: Standardize Forms spec open.
- **Issues**: SPARC boilerplate in CLAUDE.md, package.json still named create-t3-turbo

### MV (ModernVisa) — Visa Case Management (H-2A/H-2B)
- **Apps**: 3 (agency/client/admin)
- **Blockers**: Email service stubbed (Resend installed not wired), Better Auth admin plugin disabled, E2E tests empty
- **Issues**: Two worktrees of unclear status (fix-onboarding-step-validation, fix-message-security)

### WS (Wholesale Services) — Day Job Azure Bicep IaC Monorepo
- **Managed via**: Azure DevOps (not GitHub)
- **Satellite apps**: DOC, Fireball, SubmissionEngine, SalesCRM, TheBridge, CostCenter
- **P0**: Cost Center infra deployment, code at `~/dev/lu`

### NV (Nova) — This Bot
- **Issues**: Memory loss between sessions (recurring), next-gen Jira pagination bug, no photo/voice message support, 472 tool failures at startup March 26

### TC (Tribal Cities) — Burn Event Management
- **P1**: type-notifications-subsystem (80+ `as any` casts)
- **Jira**: 8 epics
- **Issues**: SPARC boilerplate in CLAUDE.md

### Nexus — Rust Session Aggregation Daemon
- **Ports**: gRPC :7400, HTTP :7401
- **Discovery**: `~/.config/nexus/agents.toml`
- **UI**: ratatui TUI
- **Status**: Session broker shipped, healthy

### CX (Cortex) — Central Projects Hub
- Houses cl/cw/co (CL :3100, CO :3101, CW :3102)
- Evolution arc: cl/cw/co → cx → nexus → nova

### FP (Flughafen PM) — Airport Design AI Consulting
- Regulatory research automation, $160-360/hr per proposal
- CT spun out of this

## Global Stack Patterns
- **T3 Turbo**: pnpm + Turborepo, Drizzle + tRPC, Next.js 15+ App Router
- **Secrets**: Doppler (dotenv-cli and @dotenvx/dotenvx BANNED)
- **UI**: shadcn/ui
- **DB imports**: `@{project}/db/client` enforced globally
- **Hosting**: Vercel (web), homelab Docker (cl/cw/co), Azure (day job)
- **Homelab**: 35+ Docker services on Arch Linux (Traefik, HA, Immich, Vaultwarden)
