# Key Decisions Log

## Architecture & Standards
- **OO is the gold standard** — all T3 projects should be modeled after it
- **Doppler replaces dotenv-cli** globally (dotenv-cli and @dotenvx/dotenvx explicitly banned)
- **DB import pattern**: `@{project}/db/client` enforced everywhere
- **Read-only snapshot approach** for cross-project status (don't formalize schema over unstable data)

## Jira Organization
- **Epic naming**: Short, concise (Auth, Registration, Vendors — not verbose)
- **OO**: 15 epics (Auth, Registration, Vendors, Sponsors, Panelists, Ambassadors, Cosplay [Runway absorbed], Comms, Facilities, Scheduling, Staff Portal, Staff, Volunteers, Tech Debt, CI/CD)
- **TC**: 8 epics
- **TL**: 6 epics (DM Portal instead of Staff Portal)
- **CT**: 10 epics (E1 + E2 never created due to pagination bug)
- **CT goes on separate LLC Jira instance** (not Leo's personal)

## CT (Civilant) Framework
- AI-assisted + human review gating (NOT fully agentic)
- E2 (AI Compliance Engine) shared ownership between James and Leo
- James's Jira display name still needed for assignment

## Nova Behavior
- Action labels required: [You], [Me], [Confirm -> Me]
- Commands only on request
- Conversational tone for chat, concise for reports
- No conversation logging to conversations.md
- Priority: oo → ct → tl → mv → ws → nv

## Communication Integration
- Teams: MS Graph via Cloud PC PowerShell, token at `~/.config/nv/graph-token.json`
- Client ID: `14d82eec-204b-4c2f-b7e8-296a70dab67e`, Tenant: `bbins.com`
- Nova cannot receive photos or voice messages via Telegram bot
