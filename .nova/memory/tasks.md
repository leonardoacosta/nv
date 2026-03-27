# Outstanding Tasks & Blockers

## Critical (P0)
1. **Cost Center infra deployment** — dev/test/stage environments need connection strings. Code at `~/dev/lu`. [Me] — Nova lacks ADO access.
2. **Reply to Yola** — Cost Center API field mapping broken, pinged 5+ times. [Me]
3. **Reply to Sanjiv** — PIPS dev environment instructions, 1+ day unanswered. [Me]
4. **ADO pipeline setup for Osman + Anand** — 7 days overdue. [Me]

## Important (Nova Infrastructure)
5. **Fix next-gen Jira pagination** — handle `isLast`-based instead of `total`-based for team-managed projects. [You]
6. **Add multi-instance Jira support** — LLC vs personal Jira. [You]
7. **CT Jira epics E1 + E2** — never landed, need to retry after pagination fix. [You]
8. **Photo/voice message support** in Telegram bot. [You]
9. **TTS voice response pipeline** — discussed but not built. [You]
10. **Nova → Nexus REST bridge** — add `GET /sessions` HTTP endpoint to nexus-agent, wire `query_nexus`. [You]
11. **Memory persistence fix** — recurring session memory loss. [You]
12. **Investigate 472 tool failures** at startup (March 26). [You]

## Project Cleanup
13. **SPARC boilerplate removal** — tc/CLAUDE.md, tl/CLAUDE.md (commands provided, execution unclear)
14. **Package.json renames** — mv, ss, tl still named "create-t3-turbo"
15. **dotenv-cli removal** — from tl, mv, ss, cl, lv, cw, co devDependencies
16. **Schema path corrections** — PATTERNS.md and per-project CLAUDE.md files
17. **MV worktree audit** — fix-onboarding-step-validation, fix-message-security branches
18. **MV email service** — Resend installed but not wired (`packages/api/src/services/email-service.ts:27`)
19. **Beads init for CT** — `bd init` not run
20. **CT scope rename** — `@fp/*` → `@ct/*` or `@civalent/*`
21. **CT `gpt-5.4` typo** — `apps/civalent/src/app/api/chat/route.ts:43`, invalid model string
22. **OO Jira epic restructuring** — existing ~40+ open epics need mapping/cleanup
23. **Bridge deployment** — blocked on REQ0278403 networking/firewall

## Backlog
24. **2-month Teams analysis for org chart** — kept timing out
25. **Wave 2 quality gates** for tc/mv/tl/ct — status unknown
26. **ft sign-up decision** — flagged pending, no resolution
