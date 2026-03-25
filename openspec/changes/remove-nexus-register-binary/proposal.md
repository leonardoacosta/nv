# Proposal: Remove nexus-register Binary

## Change ID
`remove-nexus-register-binary`

## Summary

Remove all NV-side references to the `nexus-register` binary: the deploy script sections
that install it on remote machines, any systemd unit files for it, session-hook configuration
that invokes it, and documentation passages that describe it. The binary itself lives in the
Nexus project (`~/dev/nexus`) and is removed there separately. This spec cleans up the NV
repo so no deploy artifact or doc still mentions it.

## Context

- Phase: Wave 4c — depends on `remove-nexus-crate`
- Type: refactor, Complexity: small
- Follows: `remove-nexus-crate` removes `crates/nv-daemon/src/nexus/` and the gRPC client;
  this spec removes the complementary deployment-side artifacts
- Precedes: `cleanup-nexus-config` (removes Nexus agent endpoints from `nv.toml`)

## Motivation

`nexus-register` is a binary that ran on remote machines (homelab, macbook) and registered
Claude Code agent sessions with the Nexus gRPC server. Each time a CC session started, a
`SessionStart` hook invoked `nexus-register` to announce itself. NV's deploy infrastructure
was responsible for distributing this binary and installing the hook.

After the team-agent migration (`replace-nexus-with-team-agents`) and nexus-crate removal
(`remove-nexus-crate`), NV no longer subscribes to the Nexus event stream. There is nothing
left to register with. Keeping the deploy artifacts creates a misleading picture of NV's
runtime dependencies and leaves dead install steps that will fail on a clean machine because
the binary no longer exists to copy.

The remove scope is intentionally narrow. The nexus gRPC Cargo dependencies (`tonic`, `prost`,
`prost-types`, `tonic-build`, `prost-build`), the `proto/nexus.proto` file, and `build.rs`
are all owned by `remove-nexus-crate`. This spec owns only the session-hook and deploy layer.

## What nexus-register Actually Is

`nexus-register` was compiled from a binary crate in `~/dev/nexus/` (a separate repo, not
this workspace). NV's install script was responsible for:

1. Copying the pre-built `nexus-register` binary to `~/.local/bin/` on each target machine.
2. Installing a CC `settings.json` hook entry that invoked `nexus-register` on every
   `SessionStart` event (the hook fired ~11 times per `claude -p` subprocess spawn per
   the `fix-persistent-claude-subprocess` investigation).
3. Optionally installing a systemd unit that kept a long-running registration daemon
   alive alongside `nv-daemon`.

Because the binary is from an external project, NV's codebase never contained its source.
The artifacts to remove are the install glue, not Rust source code.

## Current State (post remove-nexus-crate)

After `remove-nexus-crate` lands:

- `crates/nv-daemon/src/nexus/` — deleted (no longer exists)
- `proto/nexus.proto` — deleted
- `crates/nv-daemon/build.rs` — nexus tonic-build step removed or file deleted
- `Cargo.toml` workspace + crate-level — tonic/prost dependencies removed

What remains for this spec:

| Artifact | Location | Action |
|----------|----------|--------|
| nexus-register install step | `deploy/install.sh` | Remove section |
| nexus-register systemd unit | `deploy/nv-nexus-register.service` (if exists) | Delete file |
| CC session hook entry | `deploy/cc-hook-settings.json` or similar (if exists) | Remove or delete |
| Documentation passages | `docs/plan/nova-v7/prd.md`, `scope-lock.md` | Remove row / bullet |
| wave-plan.json entry | `docs/plan/nova-v7/wave-plan.json` | Status update only (no deletion — planning record) |
| Audit command scope | `.claude/commands/audit-nexus.md` | Remove or note as archived |
| Audit memory | `.claude/audit/memory/nexus-memory.md` | Archive or delete |

## Requirements

### Req-1: Remove nexus-register from deploy/install.sh

`deploy/install.sh` currently installs `nv-daemon`, `nv-cli`, Discord relay, and Teams relay.
If it contains a section that copies `nexus-register` to `~/.local/bin/` or installs a
`nv-nexus-register.service`, remove that section entirely. The install script must build
cleanly without any reference to `nexus-register` after this change.

If no such section exists (it was already absent when this spec executes), this task is a
no-op — confirm and mark complete.

### Req-2: Delete nexus-register systemd unit (if present)

If `deploy/nv-nexus-register.service` exists, delete it. If it was already removed by
`remove-nexus-crate`, confirm and mark complete.

### Req-3: Remove CC session hook entry (if present)

If any file in `deploy/`, `config/`, or `.claude/` contains a `settings.json` hook
definition that invokes `nexus-register` on `SessionStart` (or any other hook event),
remove that entry. The hook was what caused the ~11 startup firings per CC subprocess
described in `fix-persistent-claude-subprocess`.

If no hook entry is found, confirm and mark complete.

### Req-4: Remove nexus-register row from prd.md

`docs/plan/nova-v7/prd.md` Wave 4 table contains the row:

```
| 3 | remove-nexus-register-binary | refactor | small |
```

After this spec executes (i.e., is archived), remove that row from the table. The completed
spec becomes the historical record; the PRD table should reflect remaining work.

This task is intentionally executed at archive time, not before, to preserve the planning
record during execution.

### Req-5: Remove nexus-register bullet from scope-lock.md

`docs/plan/nova-v7/scope-lock.md` Wave 4 list contains:

```
3. **Remove nexus-register binary** -- clean up session hooks that reference it
```

Remove this bullet after the spec is archived. Same rationale as Req-4.

### Req-6: Update audit-nexus command

`.claude/commands/audit-nexus.md` scopes the audit to the `crates/nv-daemon/src/nexus/`
module. After `remove-nexus-crate` deletes that module, running `audit:nexus` would audit
nothing (or error). Either:

- Delete `audit-nexus.md` if no nexus infrastructure remains to audit, or
- Replace its scope with a tombstone note: "Nexus module removed in Wave 4 (remove-nexus-crate).
  This command is archived."

Preferred: delete the file. It adds noise to the command list once nexus is gone.

### Req-7: Archive or delete nexus audit memory

`.claude/audit/memory/nexus-memory.md` contains the 2026-03-23 audit findings for the nexus
module. After the module is removed, this file is stale context. Move it to
`.claude/audit/memory/archive/nexus-memory-2026-03-23.md` or delete it.

Preferred: move to archive subdirectory (preserves historical record without polluting active
audit memory).

## Scope

**IN**:
- `deploy/install.sh` — remove nexus-register install section if present
- `deploy/nv-nexus-register.service` — delete if present
- CC session hook config — remove nexus-register hook entry if present
- `docs/plan/nova-v7/prd.md` — remove completed spec row (at archive time)
- `docs/plan/nova-v7/scope-lock.md` — remove completed bullet (at archive time)
- `.claude/commands/audit-nexus.md` — delete
- `.claude/audit/memory/nexus-memory.md` — move to archive

**OUT**:
- The `nexus-register` source code itself (lives in `~/dev/nexus`, not this repo)
- Cargo dependencies (`tonic`, `prost`, `proto/nexus.proto`, `build.rs`) — owned by `remove-nexus-crate`
- `nv.toml` nexus agent config (`[nexus]` section) — owned by `cleanup-nexus-config`
- `crates/nv-daemon/src/nexus/` module — owned by `remove-nexus-crate`
- Any changes to `wave-plan.json` beyond status tracking
- Changes to the remaining nexus-deprecation specs in the pipeline

## Impact

| Area | Change |
|------|--------|
| `deploy/install.sh` | Remove nexus-register install section (if present) |
| `deploy/nv-nexus-register.service` | Delete (if present) |
| CC hook config file | Remove nexus-register hook entry (if present) |
| `docs/plan/nova-v7/prd.md` | Remove Wave 4 row 3 (at archive time) |
| `docs/plan/nova-v7/scope-lock.md` | Remove Wave 4 bullet 3 (at archive time) |
| `.claude/commands/audit-nexus.md` | Delete |
| `.claude/audit/memory/nexus-memory.md` | Move to `.claude/audit/memory/archive/` |

No Rust source changes. No Cargo.toml changes. No service restart required.

## Risks

| Risk | Mitigation |
|------|-----------|
| nexus-register hook entries exist in a location not found during pre-flight search | Grep for `nexus.register\|nexus_register` across entire repo before declaring clean; engineer confirms zero matches at verification |
| Deleting audit-nexus.md breaks a reference from audit-all.md or another command | Check `.claude/commands/audit-all.md` for nexus reference and remove it at the same time |
| nexus-memory.md contains findings referenced by a still-open beads issue | Check `.beads/issues.jsonl` for any open issue that cites nexus audit findings before deleting |

## Dependencies

- Depends on: `remove-nexus-crate` (must land first so nexus module is already gone)
