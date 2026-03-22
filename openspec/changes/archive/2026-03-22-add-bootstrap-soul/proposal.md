# Proposal: Add Bootstrap, Soul, Identity, and User Files

## Change ID
`add-bootstrap-soul`

## Summary

Separate Nova's monolithic system prompt into 4 concern-specific files (system-prompt, soul,
identity, user) with a Telegram-native first-run bootstrap conversation that discovers Leo's
work context, communication style, and decision patterns.

## Context
- Extends: `config/system-prompt.md`, `crates/nv-daemon/src/agent.rs`, `crates/nv-daemon/src/tools.rs`
- Related: OpenClaw's BOOTSTRAP.md + SOUL.md + IDENTITY.md pattern, competitor research

## Motivation

Nova's personality, identity, and operator preferences are currently mixed into one
`system-prompt.md`. This makes it impossible for Nova to evolve its personality independently
of operational rules, and provides no first-run onboarding experience. Splitting into 4 files
enables:

1. **Separation of concerns** — operational rules vs personality vs identity vs user prefs
2. **First-run bootstrap** — Nova discovers Leo's context via Telegram conversation
3. **Soul evolution** — Nova can adapt personality over time (with notification to Leo)
4. **Reusability** — system-prompt stays universal, soul/identity/user are instance-specific

## Requirements

### Req-1: File Separation

Extract the current monolithic `system-prompt.md` into 4 files with clear responsibilities:

- `system-prompt.md` — Dispatch test, tool rules, response format, NEVER list (operational)
- `soul.md` — Core truths, vibe, boundaries, continuity framing (personality)
- `identity.md` — Name, nature, emoji, channel, avatar (who Nova is)
- `user.md` — Leo's name, timezone, notification preference, work context, decision patterns

All 4 files live in `config/` (repo) and are symlinked to `~/.nv/`.

### Req-2: Prompt Injection Order

Agent loop loads and concatenates files in this order:

1. `system-prompt.md` — operational rules (always loaded)
2. `identity.md` — who Nova is (loaded if exists)
3. `soul.md` — personality (loaded if exists)
4. `user.md` — who Leo is (loaded if exists)
5. Memory context — recent summaries (already exists)
6. If not bootstrapped: `bootstrap.md` replaces steps 2-4

### Req-3: Bootstrap Conversation

On first run (no `~/.nv/bootstrap-state.json`), Nova conducts a thorough Telegram conversation
covering three areas:

**Work Context:**
- Active projects and which matter most
- Team structure (solo? collaborators?)
- Typical work hours and timezone
- Which Jira projects to prioritize

**Communication Style:**
- Response verbosity preference (terse vs detailed)
- Preferred digest format and frequency
- Alert sensitivity (what constitutes "urgent")
- Preferred name/address

**Decision Patterns:**
- Priority framework (speed vs quality trade-offs)
- Escalation threshold (when to bother Leo vs handle autonomously)
- What "P0" means in Leo's context

Bootstrap uses Telegram inline keyboards where appropriate (timezone, alert level) and
free-text for open-ended questions.

### Req-4: Bootstrap Completion

After the conversation, Nova:
1. Writes `identity.md` with discovered identity details
2. Writes `user.md` with Leo's preferences and context
3. Writes `soul.md` with personality derived from the conversation
4. Calls `complete_bootstrap` tool → writes `~/.nv/bootstrap-state.json`
5. All subsequent startups skip bootstrap, load the 4 files normally

### Req-5: Soul Evolution

Nova can update `soul.md` over time using `write_memory` tool targeting the soul topic.
When it does, it MUST notify Leo via Telegram: "I updated my soul.md: [what changed]."
Changes are tracked via git since `soul.md` is symlinked to the repo.

## Scope
- **IN**: 4 config files, bootstrap conversation, prompt injection refactor, `complete_bootstrap` tool, soul evolution with notification
- **OUT**: tools.md (deferred), multi-agent routing, HEARTBEAT.md system, self-destructing files

## Impact
| Area | Change |
|------|--------|
| `config/` | 4 new files: soul.md, identity.md, user.md, bootstrap.md |
| `agent.rs` | Refactor prompt loading to read 4 files + bootstrap detection |
| `tools.rs` | Add `complete_bootstrap` tool |
| `deploy/install.sh` | Add symlinks for new config files |
| `claude.rs` | Pass concatenated prompt (not just system-prompt.md) |

## Risks
| Risk | Mitigation |
|------|-----------|
| Bootstrap conversation too long | Use inline keyboards, limit to ~8 questions |
| Soul drift over time | Git tracking + mandatory notification on change |
| Prompt too large (4 files) | Budget ~500 chars per file, ~2000 total |
