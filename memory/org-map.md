# Organizational Map — Brown & Brown Wholesale IT

> Last updated: 2026-03-27 from Teams channels, DMs, standup chats, and release history
> Release history goes back to May 2023 (Fireball Releases channel)

---

## Team Structure

```
Mark Van Horn (VP, Wholesale IT)
├── Srini Tadepalli (DevOps Manager)
│   ├── Leonardo Acosta (you) — DevOps + cross-team
│   └── Vivek Gunnala — DevOps + PR reviews
│
├── Therese Lay (PM, Fireball) — your primary PM
│   ├── James Alanis — Senior Dev (IVANS, Cogitate)
│   ├── Rickey Patel — Dev (Azure Functions, logs)
│   ├── Chandni Shah — Dev (bug fixes, blob storage, B3 Admin)
│   ├── Kirk Johnson — Dev (React UI)
│   ├── Osman Meer — Dev (PRs, pipelines)
│   ├── Justin Lobato — Junior Dev (ramping up)
│   └── Sendin Hodzic — Junior Dev (ramping up)
│
├── Greg Blanford (Manager, Submission Engine)
│   ├── David Carter — Senior Dev (most active coder)
│   ├── Sarah Gleixner — Dev
│   ├── Katie Townsend — Dev (PIPS connection expert)
│   ├── Joshua Callahan — QA
│   ├── Chandrika Devi Nallamothu — QA/Dev
│   ├── Elena Ormon — Team member
│   └── Karen Rivas — Team member (ISI overlap)
│
├── Tara Bellomy — QA/Support (cross-team prod monitoring)
│
└── IT / Support
    ├── Durgasrinivas Nalla — ServiceNow/Azure access
    ├── Denise Careri — Service desk
    ├── Tess Vaessen — IT admin
    └── Brian Hawkins — DBA
```

### External / Adjacent
- **Sanjiv Surve** — PIPS developer (Futran contractor?)
- **Joe Shomphe** — PIPS automation advocate
- **Ariba** — Cost Center data
- **Eric Kinzel** — Cost Center stakeholder (disengaging)
- **Ravi Pinnamaneni** — Azure shared managed pools
- **Mphasis team** — Completed B3 Admin handoff (12 hours of meetings, departed)

---

## Project → People Matrix

### 🔥 Fireball (Core Platform)
**Release cadence:** Monthly Saturday releases, managed by Therese
**Tech:** Azure Functions, .NET 8 Core, React dashboard, App Insights, APIM

| Person | Role | Involvement |
|--------|------|-------------|
| Therese Lay | PM | Release planning, backlog, iteration reports |
| James Alanis | Dev | IVANS integrations, Cogitate, PRs, dev→test merges |
| Rickey Patel | Dev | Azure Functions, prod diagnostics, logs |
| Chandni Shah | Dev | Bug fixes, blob storage |
| Kirk Johnson | Dev | React dashboard UI |
| Osman Meer | Dev | PRs, pipelines |
| **Leonardo (you)** | DevOps | Pipelines, infrastructure, cross-team support |
| Vivek Gunnala | DevOps | PR reviews |
| David Carter | SE Dev | Cross-team PRs (keeps Dev Chat muted) |
| Sarah Gleixner | SE Dev | Cross-team PRs |
| Tara Bellomy | QA | Prod error monitoring |

**Recent releases:**
- March 21, 2026: Logging standards (save money in App Insights), React bug fixes, VNET changes
- Feb 21, 2026: Submission Scheduler React migration, Fireball Admin React migration, retry logic
- Ongoing IVANS hotfixes (LocalEdge data volume issues)

---

### 📨 Submission Engine
**Release cadence:** Coordinated with Fireball since Dec 2024
**Tech:** .NET, Azure Functions, PIPS integration, CoverForce

| Person | Role | Involvement |
|--------|------|-------------|
| Greg Blanford | Manager | Triage, delegation, change management |
| David Carter | Dev | Primary coder — logging, bug fixes, health checks |
| Sarah Gleixner | Dev | Bug investigation |
| Katie Townsend | Dev | CompLoc config, PIPS connections, IVANS |
| Joshua Callahan | QA | Sandbox testing (Morstan, Hull-Horsham) |
| Chandrika Devi Nallamothu | QA/Dev | CoverForce testing |
| Tara Bellomy | QA | Prod error monitoring, alert escalation |
| **Leonardo (you)** | DevOps | Pipeline support |

**Active issues (March 26):**
- SaveSubmission timeouts (6.3 min) — client disconnects before completion
- Stuck submissions in clearing/saving — 18 errors since 11:48 AM CT
- Katie's CompLoc update broke Local Edge and Hull Boca in test

---

### 🏗️ B3 Admin Tool
**Status:** Handoff from Mphasis complete. Onboarding Fireball team.
**Priority:** Get all devs access, then ramp Justin + Sendin

| Person | Role | Involvement |
|--------|------|-------------|
| Therese Lay | PM | Driving onboarding, assigned to you |
| Chandni Shah | Dev | Created local setup docs in Confluence |
| Justin Lobato | Dev | Learning — needs access |
| Sendin Hodzic | Dev | Learning — needs access |
| **Leonardo (you)** | DevOps | **Ensure all devs have access (Fireball + PIPS + Futran)** |

**⚠️ Your action:** Verify Mphasis handoff wasn't broken, ensure everyone can stand it up locally

---

### 💰 Cost Center API
**Status:** April release target. Ariba has final data from Eric.
**Priority:** P0 — Therese is waiting on progress

| Person | Role | Involvement |
|--------|------|-------------|
| Therese Lay | PM | Tracking for April release |
| Eric Kinzel | Stakeholder | Provided final data, dropped off meeting March 26 |
| Ariba | Data | Got final data, wants to talk to you |
| **Leonardo (you)** | Dev/DevOps | **API implementation + pipeline** |

**⚠️ Your action:** Connect with Ariba about final data, build pipeline

---

### 📧 Email Scheduler
**Status:** Chandni's blob storage fix passed testing. Needs prod pipeline.
**Priority:** April release target

| Person | Role | Involvement |
|--------|------|-------------|
| Therese Lay | PM | Tracking for April release |
| Chandni Shah | Dev | Blob storage bug fix (passed testing, pending release) |
| Sanjiv Surve | Dev | Needs local dev setup help |
| **Leonardo (you)** | DevOps | **Prod pipeline needed** |

**⚠️ Your action:** Create prod pipeline for Email Scheduler

---

### 🔧 PIPS (Policy Issuance Processing System)
**Status:** Legacy system. Manual builds/deploys. Connection stability issues.
**Tech:** .NET, Terminal Server deployments, MSI setup files

| Person | Role | Involvement |
|--------|------|-------------|
| Therese Lay | PM | Coordination |
| Katie Townsend | Dev | Connection expert, monitoring |
| Sanjiv Surve | Dev | Local dev setup (blocked — waiting on you) |
| Joe Shomphe | Automation | Wants to automate builds/deploys |
| Justin Lobato | Dev | Test DB restores |
| Tess Vaessen | Admin | Environment warnings |
| Brian Hawkins | DBA | SQL Prod, password management |
| **Leonardo (you)** | DevOps | **Help Sanjiv, attend Joe's automation meeting** |

**⚠️ Your actions:** Reply to Sanjiv, attend Joe Shomphe meeting

---

### 🌉 Bridge / Dex 360
**Tech:** React, Commission integrations

| Person | Role | Involvement |
|--------|------|-------------|
| Kirk Johnson | Dev | Audit Date Filter (React) |
| David Carter | Dev | CoverForce modal, logging |
| Greg Blanford | Manager | Change management docs |

---

### 🏢 Azure Infrastructure
**Status:** Ongoing — subscription access, shared managed pools

| Person | Role | Involvement |
|--------|------|-------------|
| Durgasrinivas Nalla | IT | ServiceNow tasks, Azure subscription access |
| Ravi Pinnamaneni | Infra | Shared managed pools meetings |
| Vivek Gunnala | DevOps | Shared managed pools |
| **Leonardo (you)** | DevOps | **Confirm access, close SNOW task with Durgasrinivas** |

---

## Your Obligation Level by Project

| Project | Level | What They Need From You |
|---------|-------|------------------------|
| Cost Center API | **PRIMARY** | Build it. April release. Connect with Ariba. |
| Email Scheduler | **PRIMARY** | Prod pipeline. Unblock Sanjiv. |
| B3 Admin | **PRIMARY** | Access provisioning for all devs. Verify Mphasis handoff. |
| PIPS Automation | **PRIMARY** | Attend Joe's meeting. Help Sanjiv with local dev. |
| Fireball | **SECONDARY** | Pipeline support, prod infra when needed |
| Submission Engine | **SECONDARY** | Pipeline support via Vivek/Greg |
| Azure Infra | **SECONDARY** | Close SNOW tasks, shared managed pools |
| Bridge/Dex 360 | **MONITOR** | Only if pipeline issues arise |

---

## Cross-Team Dynamics

### The PM Coordination Pattern
Therese (Fireball) and Greg (SE) coordinate releases since Dec 2024. Therese drives the timeline, Greg manages SE scope. Katie bridges the two teams on PIPS issues. You bridge on DevOps/pipeline issues.

### The Escalation Chain
```
Tara flags prod error → Katie/David/James investigate → Therese/Greg triage → Mark informed if major
```

### The "Sinks" Problem (your metaphor, March 17)
You told Therese: "Not a flooding rain, but a sink we can't turn off, suddenly another sink and another." This captures your current state — multiple P0/P1 items from different directions (Cost Center, B3 Admin, Email Scheduler, PIPS, Azure access) with no single one being overwhelming but the aggregate being unsustainable. Therese is in the same boat ("being pulled in a million more directions").

### Code Freeze / Release Rhythm
- **Code freeze starts Monday** (March 30)
- PRs going into test now (Kirk's React fix, James's IVANS fix)
- April release includes Cost Center and Email Scheduler changes
- Saturday release pattern — Therese posts release plans in #Releases channel

### Junior Dev Ramp-Up
Therese is deliberately investing in Justin and Sendin ("I'm making time for them to do new things, and that's a first here"). B3 Admin is the vehicle. They need access first, then guided onboarding. Chandni's Confluence docs are the starting point.

---

## Immediate Actions (Prioritized)

1. **Reply to Sanjiv** — PIPS/Email Scheduler local dev setup. Blocked since March 25. Quick win.
2. **Reply to Durgasrinivas** — Confirm Azure access works, close SNOW task. 30 seconds.
3. **B3 Admin access** — Therese asked March 26 morning. Verify all devs can access.
4. **Connect with Ariba** — Cost Center final data. April release deadline approaching.
5. **Email Scheduler prod pipeline** — Chandni's fix is waiting on this.
6. **Osman + Anand pipelines** — 7+ days overdue.
7. **Joe Shomphe meeting** — PIPS automation. Check if scheduled.

---

## Data Gaps to Fill

| Gap | Source | Priority |
|-----|--------|----------|
| Srini's management expectations | DM with Srini, 1:1s | High |
| Full PIPS team roster | More channel/standup data | Medium |
| CoverForce team members | CoverForce External team | Low |
| ISI team overlap | Karen Rivas context | Low |
| Ariba's full name and role | Cost Center meeting | Medium |
| Joe Shomphe's background | Upcoming automation meeting | Medium |
