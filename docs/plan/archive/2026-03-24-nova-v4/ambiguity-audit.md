# Ambiguity Audit -- Nova v4 PRD

## Clarity Score: 8/10

Minor ambiguities identified. All resolved in-line during PRD generation. No TBDs, no
unresolved decisions, no contradictions between artifacts.

## Findings

| # | Location | Pattern | Original | Resolution | Severity |
|---|----------|---------|----------|------------|----------|
| 1 | FR-1 | Scope clarity | "every inbound message" | Clarified: all 5 channels (Telegram, Discord, Teams, Email, iMessage). Excludes Nexus events. | Low |
| 2 | FR-2 | Undefined process | "Claude to classify" | Claude is called per-message in the orchestrator loop. Classification happens before tool dispatch. No separate model needed. | Medium |
| 3 | FR-5 | Vague threshold | "overdue threshold" | Specified: 2 hours, configurable via Settings UI and nv.toml | Low |
| 4 | FR-8 | Conditional feature | "known workflow commands" | Specified: `/apply`, `/ci:gh --fix`, `/feature`. Non-workflow sessions show elapsed time only, no progress bar. | Medium |
| 5 | FR-9 | Undefined detection | "detects server crashes" | Via Nexus health endpoint polling + systemd journal inspection. Crash = OOM, kernel panic, or unexpected restart. | Medium |
| 6 | FR-13 | Font availability | "Geist Sans/Mono" | Embedded in SPA bundle via npm package `geist`. No CDN dependency. | Low |
| 7 | FR-16 | Vague consistency | "reads memory files before every session response" | System prompt includes explicit instruction: "Read memory before answering." Enforced in system-prompt.md, not in code. | Medium |
| 8 | S3 Metric | Measurement method | "Manual audit" for obligation detection | Acceptable for single-user tool. No automated measurement infrastructure needed for v4. | Low |
| 9 | S7.1 | API design | "new API endpoints" | Endpoints listed are proposals. Final paths confirmed during implementation spec. RESTful convention assumed. | Low |
| 10 | S7.2 | Schema design | "obligations table" | Schema is a starting point. `rusqlite_migration` will version all changes. Column types match SQLite affinity rules. | Low |

## Cross-Reference Check

| Check | Result |
|-------|--------|
| Every persona has user flows | PASS -- 3 personas, 5 flows, each persona covered |
| Every v4 Must-Do has requirements | PASS -- Pillar 1 (FR-1 to FR-5), Pillar 2 (FR-6 to FR-9) |
| No requirement exceeds scope | PASS -- all FRs map to in-scope items |
| Design matches audience | PASS -- dark/dense for power user, Geist for developer aesthetic |
| Architecture supports scale | PASS -- embedded SPA, local SQLite, single user |
| No section contradicts another | PASS -- all sources aligned |

## Scoring Breakdown

| Dimension | Score | Notes |
|-----------|-------|-------|
| Completeness | 8/10 | All major areas covered. No financials (N/A). |
| Specificity | 8/10 | Concrete acceptance criteria per FR. Schema proposed. |
| Testability | 7/10 | Most criteria measurable. Some require manual audit. |
| Consistency | 9/10 | No contradictions found across 4 source artifacts. |
| Actionability | 8/10 | Ready for spec generation. API design will finalize in implementation. |

**Overall: 8/10 -- proceed to roadmap.**
