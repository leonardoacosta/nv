# Roadmap -- Nova v6

> MCP extraction: 72 tools from nv-daemon into standalone nv-tools binary.
> 6 specs across 6 sequential waves. All waves touch tools/mod.rs -- no parallelism.

---

## Tool Surface

72 tool handlers total:
- **44 stateless** (17 HTTP via reqwest, 5 process-shelling, 3 memory, etc.) -- easy extraction
- **28 daemon-coupled** (jira, teams, schedule, reminders, memory, nexus, aggregation, bash, channels, check_services) -- need SharedDeps trait

Critical coupling: every tool file imports `crate::claude::ToolDefinition`. Moving this to `nv-core` unblocks all stateless extractions.

---

## Wave 1: MCP Scaffold

| Spec | Tasks | Work |
|------|-------|------|
| `nv-tools-scaffold` | 8 | Workspace crate, ToolDefinition migration, MCP stdio skeleton |

**Key steps:**
1. Add `crates/nv-tools` to workspace
2. Move `ToolDefinition` from `nv-daemon/src/claude.rs` to `nv-core`
3. Update all `use crate::claude::ToolDefinition` across daemon
4. Create MCP server skeleton (main.rs, server.rs, registry.rs)
5. Implement stdio JSON-RPC loop (initialize, tools/list, tools/call)

**Gate:** `cargo check --workspace` clean. 1,032 tests pass.

---

## Wave 2: Extract Stateless HTTP Tools (Wave A)

| Spec | Tasks | Tools |
|------|-------|-------|
| `nv-tools-extract-wave-a` | 25 | stripe, vercel, sentry, resend, upstash |

Per-tool pattern (5 steps each):
1. Add deps to nv-tools/Cargo.toml
2. Move .rs file from daemon to nv-tools
3. Update import: `nv_core::ToolDefinition`
4. Re-export from daemon's tools/mod.rs
5. Register in nv-tools registry

**Gate:** `cargo test -p nv-daemon` -- 1,032 pass.

---

## Wave 3: Extract Stateless HTTP Tools (Wave B)

| Spec | Tasks | Tools |
|------|-------|-------|
| `nv-tools-extract-wave-b` | 35 | ha, ado, plaid, doppler, cloudflare, posthog, neon |

Same pattern as Wave 2. `neon.rs` needs `tokio-postgres` + TLS deps.

**Gate:** 1,032 tests pass.

---

## Wave 4: Extract Process-Shelling Tools

| Spec | Tasks | Tools |
|------|-------|-------|
| `nv-tools-extract-wave-c` | 25 | docker, github, web, calendar, tailscale |

`calendar.rs` needs `chrono` + `base64`. `tailscale.rs` is outside `tools/` -- may
need SharedDeps wrapping instead of direct move.

**Gate:** 1,032 tests pass.

---

## Wave 5: Daemon-Coupled Tool Wiring

| Spec | Tasks | Tools |
|------|-------|-------|
| `nv-tools-shared-deps` | 20 | jira, teams, schedule, reminders, memory, nexus, aggregation, bash, channels, check_services |

Define `SharedDeps` trait in nv-tools. Implement in nv-daemon. Wire MCP server to
dispatch daemon-coupled tools via trait object. Does NOT move these files -- just
exposes them to the MCP server.

**Gate:** 1,032 tests pass. `nv-tools tools/list` returns 72+ tools.

---

## Wave 6: Integration Test

| Spec | Tasks | Work |
|------|-------|------|
| `nv-tools-integration-test` | 8 | Smoke test, Claude Code MCP config, e2e verification |

1. `smoke.rs` integration test (spawn subprocess, initialize, tools/list, tools/call)
2. Add to `.claude/mcp.json`
3. Verify 5 representative tools via Claude Code

**Gate:** Integration tests pass. Claude Code shows nv-tools in tool list.

---

## Dependency Graph

```
Wave 1: nv-tools-scaffold (ToolDefinition migration)
  |
  v
Wave 2: extract-wave-a (5 HTTP tools)
  |
  v
Wave 3: extract-wave-b (7 HTTP tools)
  |
  v
Wave 4: extract-wave-c (5 process tools)
  |
  v
Wave 5: shared-deps (28 daemon-coupled tools)
  |
  v
Wave 6: integration-test (e2e verification)
```

All waves sequential -- every wave touches `crates/nv-daemon/src/tools/mod.rs` for re-exports.

## Conflict Map

| File | Waves |
|------|-------|
| `crates/nv-daemon/src/tools/mod.rs` | 1, 2, 3, 4, 5 |
| `crates/nv-daemon/src/claude.rs` | 1 |
| `crates/nv-core/src/lib.rs` | 1 |
| `Cargo.toml` (workspace) | 1 |
| `crates/nv-tools/Cargo.toml` | 1, 2, 3, 4, 5 |

## Summary

| Metric | Value |
|--------|-------|
| Total specs | 6 |
| Total waves | 6 |
| Total tasks | ~121 |
| Tools to extract | 72 (44 stateless + 28 daemon-coupled) |
| Existing tests | 1,032 (must not regress) |
| New binary | crates/nv-tools |
| Transport | stdio (MCP JSON-RPC) |
