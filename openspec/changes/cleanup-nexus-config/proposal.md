# Proposal: Cleanup Nexus Config

## Change ID
`cleanup-nexus-config`

## Summary

Remove all Nexus-related configuration from `config/nv.toml`, the `nv-core` config types, and
any documentation that references Nexus topology. Final housekeeping after the Nexus code and
binary have been removed in earlier Wave 4 specs.

## Context
- Phase: Wave 4d — final cleanup, depends on `remove-nexus-register-binary` and
  `update-session-lifecycle`
- Type: refactor, Complexity: trivial
- Related beads: nv-unw (nexus-duplicate-sessions-vs-team-agents)
- Extends: `crates/nv-core/src/config.rs`, `config/nv.toml`, `config/system-prompt.md`

## Motivation

After `remove-nexus-crate`, `remove-nexus-register-binary`, and `update-session-lifecycle` land,
the Nexus gRPC client is gone, the `nv-nexus-register` binary is gone, and session lifecycle is
owned by CC team agents. Two dead surfaces remain:

1. `config/nv.toml` still has a live `[nexus]` section with agent host/port entries.
2. `NexusConfig` and `NexusAgent` structs in `crates/nv-core/src/config.rs` are deserialized
   from that section and referenced in the full-config test.
3. The `default_watchdog_interval` default-value function is only used by `NexusConfig`.
4. `config/system-prompt.md` cites `query_nexus` as a tool and references Nexus events.
5. `config/bootstrap.md` prohibits `query_nexus` in bootstrap rules.

Leaving these in place causes two problems: a developer reading `nv.toml` will think Nexus is
still active, and the unused `NexusConfig` struct will eventually generate dead-code warnings
once the last call site in the daemon is removed.

## Requirements

### Req-1: Remove `[nexus]` from `config/nv.toml`

Delete the `[nexus]` table and both `[[nexus.agents]]` entries (lines 69-78 of current file).
The remaining config sections (`[daemon]`, `[agent]`, etc.) are unaffected.

### Req-2: Remove `NexusConfig` and `NexusAgent` from `nv-core`

In `crates/nv-core/src/config.rs`:
- Delete the `NexusAgent` struct (lines 444-449).
- Delete the `NexusConfig` struct (lines 451-457).
- Delete the `default_watchdog_interval` default-value function (lines 65-67), which is only
  used by `NexusConfig`.
- Remove `pub nexus: Option<NexusConfig>` from the `Config` struct (line 111).

### Req-3: Remove Nexus references from config tests

In the `parse_full_config` unit test in `config.rs`:
- Remove the `[nexus]` / `[[nexus.agents]]` TOML string from the test fixture (lines 832-836).
- Remove the `config.nexus.unwrap()` assertion block (lines 905-908).
- The `assert!(config.nexus.is_none())` in `parse_minimal_config` also goes away automatically
  once the field is removed.

### Req-4: Update `config/system-prompt.md`

- Remove `query_nexus` from the "Reads (immediate)" tool list.
- Remove `query_session` from that list if it was Nexus-specific (verify before removing).
- Remove the "Nexus events" trigger mention from the Context section.
- Remove the `[Nexus: homelab]` cite example from Response Rule 2.
- Remove "Nexus: offline" from Response Rule 3 and Rule 4.

### Req-5: Update `config/bootstrap.md`

Remove `query_nexus` from the prohibited tools list in the bootstrap rules section (line 35).

### Req-6: Daemon starts cleanly without nexus config

After these changes, running `cargo build` and starting the daemon with the updated `nv.toml`
must produce no errors, no warnings about unknown config fields, and no panic on startup.
`nv status` must return successfully.

## Scope
- **IN**: `nv.toml` nexus section, `NexusConfig`/`NexusAgent` structs, `default_watchdog_interval`,
  `Config.nexus` field, config tests, `system-prompt.md` nexus references, `bootstrap.md`
  `query_nexus` reference
- **OUT**: Any other Nexus code removal (handled by `remove-nexus-crate`), Doppler secret
  changes (no NEXUS_* secrets were found in the codebase), changes to archived specs or docs

## Impact
| Area | Change |
|------|--------|
| `config/nv.toml` | Remove `[nexus]` table and both `[[nexus.agents]]` entries |
| `crates/nv-core/src/config.rs` | Remove `NexusAgent`, `NexusConfig`, `default_watchdog_interval`, `Config.nexus` field, test fixture entries, and test assertions |
| `config/system-prompt.md` | Remove `query_nexus` tool, Nexus event trigger, Nexus cite examples |
| `config/bootstrap.md` | Remove `query_nexus` from prohibited tool list |

## Risks
| Risk | Mitigation |
|------|-----------|
| `Config.nexus` still referenced by daemon code at merge time | This spec depends on `remove-nexus-crate` completing first — that spec removes all use sites in `crates/nv-daemon/`. Enforce dependency order. |
| Removing `query_nexus` from system-prompt breaks a live agent session | Nexus is already removed from the daemon; the tool no longer exists to call. The prompt update matches reality. |
| `query_session` also needs removal | Verify the tool definition in `tools/mod.rs` before removing from prompt — if it was renamed or retained as a CC session query tool, keep it. |
