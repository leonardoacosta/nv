# BSG Relational Map — People × Projects × Responsibilities

> Last updated: 2026-03-26
> Organization: Bridge Specialty Group (BSG), subsidiary of Brown & Brown
> Org reality: Insurance company where dev/IT/PM are minority staff ("second-class citizens" — Leo's words)

---

## Team Structure

### DevOps Team (Leo's primary team)
| Person | Role | Direct Projects | ADO Ownership |
|--------|------|----------------|---------------|
| **Srini** | Manager | Wholesale Architecture, Wholesale MDR | Approver/oversight |
| **Leo** | Lead DevOps + Lead SWE | Wholesale Architecture (primary), all satellite apps, IaC | Board manager — Wholesale Architecture |
| **Vivek** | DevOps Engineer | Shared infra/pipeline work | Unknown — needs audit |

**Reporting**: Srini → Mark (VP/Director)

### Fireball Team (best-organized team in org)
| Person | Role | ADO Ownership |
|--------|------|---------------|
| **Therese** | PM (almost exclusive) | Board manager — Fireball ADO |
| **Sarah Chen** | Technical | Contributor |
| **Marcus Webb** | Technical | Contributor |

**ADO pattern**: Fireball is the gold standard — one ADO project, multiple repos. Only team doing it right.
**Status**: Last ADO activity Jan 13 — may be between sprints or on hold.
**Expansion**: Therese is now picking up greenfield + brownfield projects beyond Fireball.

### Submission Engine Team
| Person | Role | ADO Ownership |
|--------|------|---------------|
| **Greg** | PM (primary) | Board manager — Submission Engine ADO |
| Unknown devs | Technical | Unknown |

**Status**: ADO went quiet Jun 2025 — 9 months dormant. Needs status check.
**Note**: Greg also works on several other projects (unidentified).

### PIPS Team (Legacy)
| Person | Role | ADO Ownership |
|--------|------|---------------|
| **Sanjiv Surve** | Developer | Unknown |
| Unknown | Unknown | Unknown |

**Status**: 7 ADO projects (PIPS, PIPS-DB, PIPS-Letters, PIPS-Data Integrations, PIPSReports, PIPSInstall, OfficeIndexToPIPS/2.0), all last updated Jul 2025. Legacy system, likely maintenance mode.

### Unattributed Teams
These ADO projects exist but have no known owner/PM in Leo's current knowledge:
- Data Engineering Shared Platform / BSG - Data Lake → "Data Eng team" (unnamed)
- BSG - Operational Reporting → "Reporting team" (unnamed)
- Bridge-Summit, Reports, B3 Data Integration, The Bridge, Sales CRM → unknown

---

## Project → People Matrix

| Project | Leo's Role | Key People | Leo's Obligation Level |
|---------|-----------|------------|----------------------|
| **Wholesale Architecture** | Owner/primary | Srini (oversight), Vivek (shared) | PRIMARY — directly responsible |
| **Wholesale MDR** | CI/CD owner | Srini | PRIMARY |
| **Cost Center** | Infra deployment + API | Yola (consumer/stakeholder) | P0 — Yola blocked |
| **brownandbrown.its.bsi.iac.common** | IaC modules owner | — | PRIMARY — shared infra |
| **Fireball** | DevOps support | Therese (PM), Sarah, Marcus | SECONDARY — on-call for pipeline needs |
| **Submission Engine** | DevOps support | Greg (PM) | SECONDARY — dormant |
| **PIPS** | Environment support | Sanjiv (dev) | SECONDARY — Sanjiv needs env setup |
| **The Bridge** | Deployment | Unknown | SECONDARY — blocked on REQ0278403 |
| **ADO Pipelines (Osman/Anand)** | Setup | Osman, Anand | P0 — 7 days overdue |
| **Data Eng / Data Lake** | None confirmed | Unknown | MONITOR — may touch Leo's infra |
| **Bridge-Summit** | None confirmed | Unknown | UNKNOWN — investigate |
| **Reports / Op Reporting** | None confirmed | Unknown | MONITOR |
| **Sales CRM** | None confirmed | Unknown | MONITOR |

---

## Cross-Team Dynamics & Terminology Risk

### The Silo Problem (Leo's core frustration)
- Teams make half-decisions independently
- Same concepts get different names across teams
- Different urgency levels for the same deliverables
- Different tech literacies among project owners → inconsistent ADO structure
- PMs (Therese, Greg) translate between groups → creates terminology drift

### Known Terminology Gaps (to be filled as data comes in)
| Concept | Team A calls it | Team B calls it | Unified term |
|---------|----------------|----------------|-------------|
| *TBD* | *Need Teams/DM access to map these* | | |

### ADO Organization Anti-Patterns
- **Most teams**: One ADO project per repository (fragmented, hard to track)
- **Fireball (correct)**: One ADO project, multiple repos inside it
- **PIPS (extreme fragmentation)**: 7+ separate ADO projects for what is essentially one system

---

## Proactive Anticipation — How Leo Can Get Ahead

### Immediate (this week)
1. **Yola** — Reply with Cost Center API fix timeline. She's pinged 5+ times. Every day of silence erodes trust.
2. **Sanjiv** — Send PIPS dev environment instructions. Quick win, 1 message, unblocks him.
3. **Osman + Anand** — Set up ADO pipelines. 7 days overdue. Even a "this week" ETA buys goodwill.

### Short-term (next 1-2 weeks)
4. **Therese** — When her new greenfield/brownfield projects spin up, proactively offer a DevOps onboarding package (CI/CD template, environment setup, IaC scaffold). Don't wait for her to ask.
5. **Greg** — Check Submission Engine status. If it's alive, it may need pipeline maintenance. If dead, Greg is fully allocated elsewhere — find out where.
6. **Srini** — Provide a clean cross-project status summary he can relay to Mark without editing. Make Srini's job easier = Leo gets more autonomy.

### Medium-term (next month)
7. **Bridge-Summit** — Identify the owner. It's active (Mar 12) and unknown. Could be a new obligation Leo doesn't know about yet.
8. **Data Engineering team** — Map who they are. Data Eng Shared Platform + BSG Data Lake are active. If they need infra, they'll come to Leo eventually.
9. **ADO standardization proposal** — Use Fireball as the case study, propose org-wide ADO project structure to Srini/Mark. Solves the fragmentation problem and positions Leo as the process leader.

### Ongoing
10. **Terminology map** — As Teams DM/channel access is restored, start cataloging when different teams call the same thing different names. This becomes Leo's secret weapon for cross-team translation.

---

## Data Gaps — What I Still Need

| Gap | Source Needed | Action |
|-----|--------------|--------|
| Mark's priorities and communication style | Teams DMs, Outlook, meetings | Restore Teams Chat.Read permission |
| Srini's preferred reporting format | Direct observation | Ask Leo or observe |
| Vivek's specific project ownership | ADO work item assignment data | Pull ADO work items per project |
| Greg's other projects beyond Submission Engine | Teams messages, ADO | Restore messaging access |
| Therese's new greenfield/brownfield projects | Teams messages | Restore messaging access |
| Bridge-Summit owner | ADO project details | Pull ADO contributors |
| Data Eng team members | ADO + Teams | Cross-reference |
| Full terminology map | High-volume Teams channel messages | Need >20 msg limit or DM access |
| Communication style profiles (all) | 2+ weeks of message history | Need expanded Teams/Outlook access |
