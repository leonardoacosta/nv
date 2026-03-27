# BSG / Brown & Brown — People Profiles

> Last updated: 2026-03-26
> Source: Telegram chat history, ADO project audit, Teams chat scan
> Confidence: Moderate — profiles will deepen as Teams DM/channel access is restored

---

## Org Chart

```
Mark (VP/Director — IT/Dev leadership)
└── Srini (Manager — DevOps + Dev)
    ├── Leo Acosta (Lead DevOps + Lead SWE) ← YOU
    └── Vivek (DevOps Engineer — peer)

Therese (PM — formerly Leo's manager, now lateral/below Srini)
Greg (PM — Submission Engine primary, multi-project)

Sanjiv Surve (Developer — PIPS)
Yola (Stakeholder/User — Cost Center)
Osman (Developer/Ops — needs ADO pipelines)
Anand (Developer/Ops — needs ADO pipelines)
Sarah Chen (Technical — Fireball team)
Marcus Webb (Technical — Fireball team)
```

---

## Individual Profiles

### Mark
- **Role**: VP or Director level — Srini's boss, top of Leo's reporting chain
- **Projects**: Oversight of all BSG IT/Dev — not hands-on
- **Communication style**: Unknown — no direct message data yet
- **Leo's leverage**: Mark is the escalation path. Understanding his priorities lets Leo frame asks in terms Mark cares about (cost, risk, timelines)
- **Anticipation**: When Leo needs budget, headcount, or cross-team authority, Mark is the sponsor. Prepare executive-ready summaries before escalating
- **Gaps**: No direct interaction data. Need Teams/Outlook access to profile

### Srini
- **Role**: Leo's current direct manager, manages DevOps team
- **Projects**: Wholesale Architecture (primary), Wholesale MDR, broader IT oversight
- **Reporting**: Reports to Mark
- **Communication style**: Unknown — insufficient message data
- **Leo's leverage**: Srini is the daily alignment point. Keeping him informed proactively reduces surprise escalations
- **Anticipation**: Srini likely needs status summaries he can relay upward to Mark. Provide him with clean, pre-formatted project status without being asked
- **Gaps**: Communication style, meeting cadence, preferred reporting format

### Vivek
- **Role**: DevOps Engineer — Leo's peer under Srini
- **Projects**: Shared DevOps responsibilities, likely overlaps on Wholesale Architecture and IaC
- **Communication style**: Unknown
- **Leo's leverage**: Peer coordination — division of labor on pipeline/infra work
- **Anticipation**: When Leo is overloaded, Vivek is the natural hand-off partner. Identify which ADO pipeline or infra tasks Vivek could own independently
- **Gaps**: Which specific pipelines/projects Vivek owns vs. Leo, communication patterns

### Therese (Teres)
- **Role**: Project Manager — formerly Leo's direct manager, now lateral to Srini
- **Projects**: Fireball (primary PM, almost exclusive), expanding into greenfield + brownfield projects
- **Team**: Works with Sarah Chen and Marcus Webb on Fireball
- **Communication style**: PM-oriented, likely task/deadline driven
- **Leo's leverage**: Therese manages Fireball — the ONE ADO project organized correctly (one project, multiple repos). She's a potential ally for standardizing other teams' ADO structure
- **Anticipation**:
  - Fireball ADO was last active Jan 13 — if it picks back up, Therese will need pipeline/deployment support from Leo
  - She's expanding into new projects → will need DevOps onboarding for those projects (CI/CD, environments, IaC)
  - As someone who used to manage Leo, she understands his workload — potential advocate
- **Risk**: Cross-team terminology drift — Therese translates between stakeholder groups, may introduce inconsistent naming

### Greg
- **Role**: Project Manager — Submission Engine primary, multi-project
- **Projects**: Submission Engine (PM), plus several other unidentified projects
- **Communication style**: Unknown
- **Leo's leverage**: Greg straddles multiple projects — he's a signal source for what's happening across teams
- **Anticipation**:
  - Submission Engine ADO went quiet June 2025 — 9 months dormant. Either completed, stalled, or migrated. Clarify status
  - When Greg starts new projects, he'll need the same DevOps scaffolding (pipelines, environments, repos)
  - Greg and Therese are the two key PMs — if they adopt consistent terminology, it cascades to their teams
- **Risk**: Same cross-team terminology drift risk as Therese. Different PM styles may create conflicting conventions

### Yola
- **Role**: Stakeholder/end-user — Cost Center application
- **Projects**: Cost Center (primary consumer of the API)
- **Communication style**: Persistent — has pinged Leo 5+ times about Cost Center API field mapping
- **Leo's leverage**: Yola is a direct customer of Leo's output. Keeping her unblocked reflects well on the team
- **Anticipation**:
  - **P0**: Cost Center API field mapping is broken. Yola is blocked and escalating. This is Leo's #1 people-obligation right now
  - Once API fields are fixed, Yola will likely have follow-up validation/testing needs
  - Future: any Cost Center feature changes will route through Yola for UAT
- **Action needed**: [Me] Reply to Yola with fix timeline

### Sanjiv Surve
- **Role**: Developer — PIPS ecosystem
- **Projects**: PIPS (legacy system — 7+ ADO projects, all dormant since Jul 2025)
- **Communication style**: Patient but waiting — sent unanswered DM Mar 25 asking if Leo can build/run PIPS locally
- **Leo's leverage**: Sanjiv is blocked on environment setup. Quick response builds trust
- **Anticipation**:
  - **P0**: Sanjiv needs PIPS dev environment instructions. 1+ day unanswered
  - PIPS is legacy (7 projects, all last touched Jul 2025) — Sanjiv may be maintaining/modernizing it
  - If PIPS is being modernized, Leo's DevOps expertise (CI/CD, IaC, modern deployment) becomes critical
  - If PIPS is just being maintained, Sanjiv needs minimal support — just environment docs
- **Action needed**: [Me] Reply to Sanjiv with PIPS dev environment instructions

### Osman
- **Role**: Developer or Ops — needs ADO pipeline setup
- **Projects**: Unknown — awaiting pipeline configuration from Leo
- **Communication style**: Unknown
- **Leo's leverage**: Osman is 7+ days overdue for ADO pipeline setup
- **Anticipation**:
  - **P0**: ADO pipeline setup is 7 days overdue. Osman is likely blocked or working around it manually
  - Once pipelines are set up, Osman will need brief onboarding on how to use/trigger them
- **Action needed**: [Me] Set up ADO pipelines for Osman + Anand

### Anand
- **Role**: Developer or Ops — needs ADO pipeline setup (same request as Osman)
- **Projects**: Unknown — likely same team/project as Osman
- **Communication style**: Unknown
- **Anticipation**: Same as Osman — blocked on pipeline setup, 7 days overdue
- **Action needed**: [Me] Set up ADO pipelines (shared with Osman)

### Sarah Chen
- **Role**: Technical contributor — Fireball team
- **Projects**: Fireball
- **Communication style**: Unknown
- **Leo's leverage**: Sarah is on the team with the best ADO organization. Understanding her workflow could help replicate Fireball's patterns elsewhere
- **Gaps**: Role specifics, seniority, what she contributes technically

### Marcus Webb
- **Role**: Technical contributor — Fireball team
- **Projects**: Fireball
- **Communication style**: Unknown
- **Leo's leverage**: Same as Sarah — Fireball team member
- **Gaps**: Role specifics, seniority, technical focus area

### James
- **Role**: Developer — Civilant/CT (Leo's LLC project, NOT Brown & Brown)
- **Projects**: CT — built 20+ regulatory collectors, owns E1 (Regulatory Corpus), shared E2 (AI Compliance Engine) with Leo
- **Context**: External collaborator on Priceless LLC work, not a BSG employee
- **Note**: Included here for completeness since James intersects with Leo's work context

---

## Unknown / Needs Investigation

These ADO projects have unidentified owners or teams:
- **Bridge-Summit** (last active Mar 12) — new, never discussed
- **Reports** (last active Mar 17) — unknown owner
- **BSG - Operational Reporting** (last active Feb 16) — reporting team?
- **B3 Data Integration** (last active Feb 3) — unknown
- **Data Engineering Shared Platform** (last active Mar 5) — Data Eng team
- **BSG - Data Lake** (last active Mar 10) — Data team
- **The Bridge** (last active Oct 2) — unknown PM
- **Sales CRM** (last active May 23) — unknown PM

These represent gaps in Leo's org awareness. Each likely has a PM or lead who Leo may need to coordinate with.
