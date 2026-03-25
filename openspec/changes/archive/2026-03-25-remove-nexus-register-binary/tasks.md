# Implementation Tasks

<!-- beads:epic:nv-2hjy -->

## Batch 1: Pre-Flight Search

- [ ] [1.1] [P-1] Grep entire repo for `nexus.register\|nexus_register\|nexus-register` (excluding `docs/plan/`, `openspec/`, `.beads/`) — confirm which deploy artifacts reference it and which do not; log findings as comments on subsequent tasks [owner:api-engineer]
- [ ] [1.2] [P-1] Check `.beads/issues.jsonl` for any open issue citing nexus audit findings — if found, note the issue IDs before proceeding with audit memory archival [owner:api-engineer]

## Batch 2: Deploy Artifact Cleanup

- [ ] [2.1] [P-1] In `deploy/install.sh` — remove any section that copies `nexus-register` to `~/.local/bin/` or installs a `nv-nexus-register.service`; if no such section exists, confirm as no-op and mark complete [owner:api-engineer]
- [ ] [2.2] [P-1] Delete `deploy/nv-nexus-register.service` if the file exists; if already absent (removed by `remove-nexus-crate`), confirm as no-op and mark complete [owner:api-engineer]
- [ ] [2.3] [P-1] Search `deploy/`, `config/`, and `.claude/` for any `settings.json` or hook config that invokes `nexus-register` on `SessionStart` or any CC hook event — remove the hook entry; if not found, confirm as no-op and mark complete [owner:api-engineer]

## Batch 3: Audit Infrastructure Cleanup

- [ ] [3.1] [P-2] Delete `.claude/commands/audit-nexus.md` — the nexus module no longer exists after `remove-nexus-crate`; running this command against a deleted module is a no-op at best and confusing at worst [owner:api-engineer]
- [ ] [3.2] [P-2] In `.claude/commands/audit-all.md` — remove the nexus row from the domains table (line `| nexus | single | gRPC client, query, notifications | /audit:nexus |`) and remove the `Wave 3` nexus bullet (`- nexus (remote agent sessions)`); update domain count from 8 to 7 in the file header [owner:api-engineer]
- [ ] [3.3] [P-2] Move `.claude/audit/memory/nexus-memory.md` to `.claude/audit/memory/archive/nexus-memory-2026-03-23.md` — preserves the historical audit record without polluting active audit memory; create the `archive/` subdirectory if it does not exist [owner:api-engineer]

## Batch 4: Documentation Cleanup

- [ ] [4.1] [P-3] In `docs/plan/nova-v7/prd.md` — remove the Wave 4 table row `| 3 | remove-nexus-register-binary | refactor | small |` and renumber rows 4 and 5 to 3 and 4 [owner:api-engineer]
- [ ] [4.2] [P-3] In `docs/plan/nova-v7/scope-lock.md` — remove the Wave 4 bullet `3. **Remove nexus-register binary** -- clean up session hooks that reference it` and renumber bullets 4 and 5 to 3 and 4 [owner:api-engineer]

## Verify

- [ ] [5.1] Grep for `nexus.register\|nexus_register\|nexus-register` across the full repo (excluding `openspec/` archive and `docs/plan/` planning records) — confirm zero matches [owner:api-engineer]
- [ ] [5.2] Confirm `deploy/install.sh` references no binaries or services beyond `nv-daemon`, `nv-cli`, `nv-discord-relay.service`, and `nv-teams-relay.service` [owner:api-engineer]
- [ ] [5.3] Confirm `.claude/commands/audit-nexus.md` does not exist [owner:api-engineer]
- [ ] [5.4] Confirm `.claude/audit/memory/nexus-memory.md` does not exist (moved to archive) [owner:api-engineer]
- [ ] [5.5] Confirm `deploy/nv-nexus-register.service` does not exist [owner:api-engineer]
- [ ] [5.6] cargo build --workspace passes (no Rust changes expected; confirms `remove-nexus-crate` precondition was met) [owner:api-engineer]
