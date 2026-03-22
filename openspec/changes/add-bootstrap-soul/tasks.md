# Implementation Tasks

<!-- beads:epic:TBD -->

## Config Files

- [ ] [1.1] [P-1] Extract personality from system-prompt.md into config/soul.md (core truths, vibe, boundaries, continuity) [owner:api-engineer]
- [ ] [1.2] [P-1] Create config/identity.md template (name: Nova, nature: operations daemon, emoji: TBD, channel: Telegram) [owner:api-engineer]
- [ ] [1.3] [P-1] Create config/user.md template (name: TBD, timezone: TBD, notification_level: TBD, work context: TBD) [owner:api-engineer]
- [ ] [1.4] [P-1] Create config/bootstrap.md — thorough first-run conversation script covering work context, communication style, decision patterns (~8 questions) [owner:api-engineer]
- [ ] [1.5] [P-2] Update config/system-prompt.md — remove personality/soul content, keep only operational rules (dispatch, tools, response format, NEVER) [owner:api-engineer]

## Rust Implementation

- [ ] [2.1] [P-1] Add load_identity(), load_soul(), load_user() functions in agent.rs — read from ~/.nv/ with fallback to defaults [owner:api-engineer]
- [ ] [2.2] [P-1] Add check_bootstrap_state() function — reads ~/.nv/bootstrap-state.json, returns bool [owner:api-engineer]
- [ ] [2.3] [P-2] Refactor build_system_context() — concatenate system-prompt + identity + soul + user (or bootstrap.md if not bootstrapped) [owner:api-engineer]
- [ ] [2.4] [P-2] Add complete_bootstrap tool to tools.rs — writes bootstrap-state.json with timestamp, registers as available tool [owner:api-engineer]
- [ ] [2.5] [P-2] Add update_soul tool to tools.rs — writes soul.md + sends notification to Telegram ("I updated my soul: [summary]") [owner:api-engineer]
- [ ] [2.6] [P-3] Update agent.rs agent loop — pass concatenated context to ClaudeClient instead of just system_prompt [owner:api-engineer]
- [ ] [2.7] [P-3] Wire bootstrap detection into agent loop — if not bootstrapped, inject bootstrap.md into first prompt context [owner:api-engineer]

## Deploy

- [ ] [3.1] [P-1] Update deploy/install.sh — add symlinks for soul.md, identity.md, user.md from config/ to ~/.nv/ [owner:api-engineer]
- [ ] [3.2] [P-2] Update deploy/install.sh — copy bootstrap.md to ~/.nv/ on fresh install (not symlinked — consumed once) [owner:api-engineer]

## Verify

- [ ] [4.1] cargo build passes with all changes [owner:api-engineer]
- [ ] [4.2] cargo clippy -- -D warnings passes [owner:api-engineer]
- [ ] [4.3] cargo test — existing tests still pass + new tests for bootstrap state, file loading [owner:api-engineer]
- [ ] [4.4] [user] Manual test: delete bootstrap-state.json, restart Nova, verify bootstrap conversation starts on Telegram [owner:api-engineer]
