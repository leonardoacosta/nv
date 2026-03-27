# People — Brown & Brown Wholesale IT

> Last updated: 2026-03-27 from Teams DMs, channels, standups, and release comms
> Sourced from: Dev Chat, SE Standup, Fireball Standup, Fireball Releases, WholesaleIT, DMs with Therese/Sanjiv/Durgasrinivas

---

## Your Management Chain

### Mark Van Horn — VP, Wholesale IT
- **Role:** VP overseeing all wholesale technology teams (Fireball, Submission Engine, PIPS)
- **Projects:** All — oversight level
- **Style:** Encouraging, praise-forward ("You ROCK!"). Listens to recordings when he misses meetings. Delegates to Greg/Therese. Will step in to suggest devs "jump on a call" when threads stall.
- **How to work with him:** Keep him informed at summary level. He notices team wins — surface them. Don't escalate unless Greg/Therese can't resolve.

### Srinivasa Rao Tadepalli (Srini) — Your Direct Manager
- **Role:** DevOps manager, your direct report line
- **Projects:** Infrastructure, pipelines, shared managed pools, community of practice
- **Style:** Still figuring out management cadence post-reorg. Therese forwards meeting invites to him. Involved in "spec driven development community of practice" initiative.
- **How to work with him:** Proactively update him on your plate. He's ramping into the role — your clarity helps him.
- **Last seen:** Azure Shared Managed Pools meeting (March 25)

### Vivek Gunnala — Your Peer (DevOps)
- **Role:** DevOps engineer, same team as you under Srini
- **Projects:** Fireball (PR reviews, pipeline work), Submission Engine (PR reviews), Azure Shared Managed Pools
- **Style:** Responsive to PR reviews, brief communicator. 1-year anniversary celebrated May 2024.
- **How to work with him:** He's your go-to for PR approvals. Reliable but not chatty — ping directly when you need reviews.

---

## Fireball Team

### Therese Lay — PM, Fireball (PRIMARY relationship)
- **Role:** Project Manager for Fireball. Expanding scope to Cost Center, Email Scheduler, B3 Admin, and PIPS automation. Manages releases, standups, iteration reports, backlog refinement.
- **Projects:** Fireball (primary), Cost Center API, Email Scheduler, B3 Admin, PIPS Automation
- **Style:** High-context communicator. Uses DMs for tasking and status checks. Empathetic ("things are tricky right now"). Will type long context dumps over lunch. Sends iteration reports, release plans in detail. Renamed "Fireball" chat → "Dev Chat" and added junior devs. Manages cross-team coordination with SE.
- **Communication pattern:** Morning pings for status ("G'morning! Update on B3 admin access?"). Will chase if no response. Understands you're stretched thin.
- **Key asks of you (as of March 26):**
  1. B3 Admin — ensure all devs (Fireball + PIPS + Futran) have access
  2. Cost Center API — targeted for April release, Ariba has final data from Eric
  3. Email Scheduler — pipeline for prod needed (Chandni's blob fix passed testing)
  4. PIPS Automation — meeting with Joe Shomphe to automate builds/deploys
  5. Community of Practice — check if you're supposed to join spec-driven dev group
- **How to anticipate her needs:** She's your #1 internal customer. Proactive updates save her from chasing. When she asks "any updates?", she's already behind on reporting upstream.

### James Alanis — Senior Developer
- **Role:** Senior dev, Fireball team. IVANS integrations, Cogitate cancellations. PR author and reviewer.
- **Projects:** Fireball (IVANS, Cogitate), Submission Engine (cross-team PRs)
- **Style:** Dry humor ("leave it to microsoft to push updates and erase our custom query history"). Quick to investigate prod issues. Merges dev→test branches. Will ask if you're in the community practice meetings.
- **Recent work:** IVANS policy number formatting (PR 18132), Cogitate cancellations (PR 18097), dev-to-test merges
- **How to work with him:** Technical peer. He'll investigate prod issues fast. Good sounding board for architecture questions.

### Rickey Patel — Developer
- **Role:** Developer, Fireball team. Azure Function investigation, logs analysis, prod triage.
- **Projects:** Fireball (Azure Functions, prod diagnostics)
- **Style:** Analytical — digs into logs and App Insights. Found the SaveSubmission 6.3-min timeout. Posts JSON payloads for debugging.
- **Recent work:** Identified client timeout on SaveSubmission, traced through App Insights logs
- **How to work with him:** Good for deep-dive debugging. Give him access to logs and he'll find root cause.

### Chandni Shah — Developer
- **Role:** Developer, Fireball team. Bug fixes, blob storage, B3 Admin local setup docs.
- **Projects:** Fireball (bug fixes), Email Scheduler (blob storage fix), B3 Admin
- **Style:** Proactive — volunteers to help with prod issues ("you can pull me in too!"). Created B3 Admin local setup documentation in Confluence.
- **Recent work:** Blob storage bug fix (passed testing, pending release), B3 Admin setup guide, prod issue triage
- **How to work with him:** She's already ramping on B3 Admin — leverage her setup docs when onboarding Justin/Sendin.

### Kirk Johnson — Developer
- **Role:** Developer, Fireball team. React UI fixes.
- **Projects:** Fireball (React dashboard)
- **Style:** Brief communicator. PRs directly to dev/test. Occasionally has schedule conflicts.
- **Recent work:** Dex 360 Audit Date Filter fix (PR 18141)

### Osman Meer — Developer
- **Role:** Developer, Fireball team. PR reviews, pipeline setup.
- **Projects:** Fireball (PRs, pipelines)
- **Style:** Concise ("sounds good", "it completed", "needs one pr approver")
- **⚠️ ACTION:** Pipeline setup with Anand — 7+ days overdue per your task tracker

### Justin Lobato — Junior Developer (ramping up)
- **Role:** Junior dev being given new responsibilities by Therese. Previously on PIPS/Wholesale Applications.
- **Projects:** B3 Admin (learning), PIPS (test DB restores)
- **Context:** Therese is deliberately making time for Justin and Sendin to grow ("that's a first here"). Needs B3 Admin access and onboarding.

### Sendin Hodzic — Junior Developer (ramping up)
- **Role:** Junior dev, same ramp-up path as Justin. Legacy PIPS knowledge from Wholesale Applications era.
- **Projects:** B3 Admin (learning), PIPS (legacy)
- **Context:** Has historical PIPS knowledge from 2021-2022 era. Being brought into Dev Chat for cross-team exposure.

---

## Submission Engine Team

### Greg Blanford — Manager, Submission Engine
- **Role:** Manager/lead for Submission Engine team. Triage and delegation. Change management documentation.
- **Projects:** Submission Engine (primary), Bridge Summit
- **Style:** Asks QA to investigate and document. Delegates to David/Sarah. "Are you able to see if it's happening to several or is this a one off?" — methodical triage approach.
- **How to work with him:** He manages the SE team independently. Interface through shared standups and cross-team PRs.

### David Carter — Senior Developer, Submission Engine
- **Role:** Most active developer across all teams. Logging improvements, bug fixes, PR machine.
- **Projects:** Submission Engine (primary — bug fixes, CoverForce modal, phone formatting, health checks, OIP functions)
- **Style:** Prolific coder. Posts PRs with detailed descriptions. Notifies QA directly when fixes hit test ("this will be in Test in 10 mins"). Asks Vivek for reviews. Found DescriptionOfOperations null bug.
- **Recent PRs:** 18057 (logging + phone formatting), bug fixes for scrolling, email submissions, DescriptionOfOperations null issue
- **How to work with him:** He's in your Dev Chat but keeps it muted (SE stuff doesn't overlap). Occasionally needs Vivek's PR reviews.

### Sarah Gleixner — Developer, Submission Engine
- **Role:** Developer, SE team. Bug investigation and triage.
- **Projects:** Submission Engine
- **Style:** Tagged by Tara on error alerts. Quieter in chat.

### Katie Townsend — SE/PIPS Connection Expert
- **Role:** Submission Engine developer with deep PIPS integration knowledge. CompLoc config, IVANS hotfixes.
- **Projects:** Submission Engine (primary), PIPS (connection stability), IVANS
- **Style:** Technical and precise. Updates CompLoc configs that affect test environments. Runs standup when Therese is out. Knows the PIPS connection issues deeply.
- **Key quote:** "I believe additional monitoring was or will be added for PIPS to pin point the cause of these issues."
- **How to work with her:** She's the bridge between SE and PIPS. When PIPS connection issues arise, she's the expert.

### Joshua Callahan — QA
- **Role:** QA for Submission Engine. Sandbox testing.
- **Projects:** Submission Engine (testing — Morstan, Hull-Horsham, sandbox environments)
- **Style:** Reports issues clearly. Tests in sandbox and test environments. Flags stuck submissions.

### Chandrika Devi Nallamothu — QA/Dev
- **Role:** QA/testing for Submission Engine. Occasionally has schedule conflicts.
- **Projects:** Submission Engine (testing, CoverForce)
- **Recent:** Found extracting/effective date bug (01/01/1753) after David's changes

### Elena Ormon — Team Member
- **Role:** SE team member. Limited data — appears in standup attendance.
- **Projects:** Submission Engine

### Karen Rivas — Team Member
- **Role:** SE team member. ISI sprint overlap.
- **Projects:** Submission Engine, ISI (cross-team)

---

## QA / Support

### Tara Bellomy — Production Error Monitor
- **Role:** QA/Support. Primary production error alerter across Fireball and Submission Engine.
- **Projects:** Fireball (prod monitoring), Submission Engine (prod monitoring), PIPS (connection alerts)
- **Style:** HIGH VOLUME alerter. Posts error messages verbatim with submission IDs and stack traces. Fast escalation — "I'm still getting these", "suddenly getting a slew of". Will make bug stories when asked.
- **Pattern:** Tara flags → Katie/David/Therese triage → dev investigates
- **How to work with her:** She's your early warning system. When Tara posts, something is actually wrong in prod.

---

## PIPS / Automation

### Sanjiv Surve — PIPS Developer (⚠️ WAITING ON YOU)
- **Role:** PIPS developer needing local dev environment setup help
- **Projects:** PIPS, PIPS Email Scheduler
- **Style:** Polite, formal ("Good Afternoon Leonardo Acosta"). Checks in periodically. Has been waiting since March 25.
- **Last message:** "Just checking to see if you are able to build from git dev repo and deploy, run/test PIPS or PIPS Email Scheduler either from your local or dev environment."
- **⚠️ ACTION:** Reply to Sanjiv — he's blocked on local dev setup

### Joe Shomphe — PIPS Automation
- **Role:** Wants to help automate PIPS builds and deploys. Everything currently super manual (MSI setup files, manual installs on terminal servers).
- **Projects:** PIPS Automation
- **Context:** Reached out to Therese. 30-min meeting scheduled (forwarded to Srini). You may need to attend.

### Ariba — Cost Center
- **Role:** Got final data from Eric for Cost Center API
- **Projects:** Cost Center API
- **Context:** Was going to talk to you about the data. Therese flagged this March 23.

### Eric Kinzel — Cost Center
- **Role:** Cost Center stakeholder. Dropped off the Cost Center meeting March 26.
- **Projects:** Cost Center API
- **Context:** Provided final data to Ariba. Crashed prod-web-dashboard once in 2024 (invalid direct bill upload). Disengaging — may need re-engagement.

---

## IT / Infrastructure

### Durgasrinivas Nalla — IT/ServiceNow (⚠️ WAITING ON YOU)
- **Role:** IT support, Azure subscription access provisioning via ServiceNow tasks
- **Projects:** Infrastructure (Azure subscription access)
- **Style:** Very consistent communication pattern: "Hi Leonardo" → "Morning" → "regarding this task TASKXXXXXXX" → "could you please [verify/help/confirm]". Persistent but patient.
- **Active tasks:** TASK0674259 (Azure subscription access — you provided subscription IDs March 23, access given, he wants to close it)
- **History:** Has been your IT contact since at least Sept 2025. Multiple SNOW tasks completed together.
- **⚠️ ACTION:** Confirm access works and let him close the task

### Ravi Pinnamaneni — Azure Shared Managed Pools
- **Role:** Involved in Azure Shared Managed Pools meetings
- **Projects:** Infrastructure (shared managed pools)
- **Context:** Schedules meetings with you and Vivek. Rescheduled one session March 16.

---

## Cross-Team / Legacy

### Tammie Miller — Former Wholesale Apps Manager (legacy)
- **Role:** Previously managed Wholesale Applications team. Handled PIPS, service desk, compliance. Sent heartfelt year-end messages.
- **Projects:** PIPS (legacy), Wholesale Applications (legacy)
- **Status:** Legacy contact — team restructured under current org

### Daniel Burns — Former Developer
- **Role:** Identified PIPS Invoice generation issue in Hull Terminal Server (May 2024)
- **Projects:** PIPS (legacy)

### Brian Hawkins — DBA
- **Role:** Database administration. PIPS SQL Prod password management.
- **Projects:** PIPS (database), Fireball (DB issues)
- **Context:** Therese reached out to him about DB issues during the SaveSubmission timeout incident

### Tess Vaessen — IT/Admin
- **Role:** Team admin. Warned about Test PIPS UI pointing to production.
- **Projects:** PIPS (environment management)

### Jon DeAngelo — IT Admin
- **Role:** Created Development Team channel, gave Therese/Greg owner permissions on WholesaleIT team (April 2024)

### Denise Careri — Support/Service Desk
- **Role:** Routes tickets, reports PIPS access issues, escalates user problems
- **Projects:** WholesaleIT (service desk), PIPS (user access)

### Nikita Garcia — Former Developer
- **Role:** SendGrid migration, InsCipher feed fixes (2023)
- **Projects:** Fireball (legacy — SendGrid, InsCipher)

### Troy Booth — Former Developer
- **Role:** Email scheduling, TLS compliance (2022-2023)
- **Projects:** PIPS (legacy), Email Scheduler (legacy)

### Matthew Morales (Matt) — Former Developer
- **Role:** Worked with you on internal docs and branching strategy (April 2024)
- **Projects:** Fireball (docs, branching conventions)
- **Context:** You co-authored the feat/hotfix branching convention with him, Therese, and Vivek

---

## Data Gaps

| Person | What's Missing | How to Fill |
|--------|---------------|-------------|
| Mark Van Horn | Management priorities, 1:1 patterns | Observe over time |
| Srini | Communication style, expectations of you | DM history, 1:1s |
| Vivek | Technical opinions, workload | More PR/channel data |
| Sarah Gleixner | Communication style, current work | SE standup messages |
| Karen Rivas | Role details, ISI context | More standup data |
| Elena Ormon | Role, responsibilities | More standup data |
| Joe Shomphe | Background, automation vision | Upcoming meeting |
| Ariba | Full name, role details | Cost Center meeting |
