# Scope Lock -- Nova v6

## Vision

Nova v6 extracts all 60+ tool implementations from nv-daemon into a standalone MCP server
binary (`nv-tools`). This is Phase 2 of the cc-native-nova migration (nv-k86), enabling
Nova to eventually run as a native Claude Code session instead of a Rust daemon.

## Target Users

Leo (sole user) -- needs the same tools accessible via both the existing daemon AND Claude Code
sessions via MCP, without duplicating implementation code.

## Domain

**In scope:**
- New workspace crate: `crates/nv-tools` -- MCP server binary (stdio transport)
- Extract tool handler logic from `tools/*.rs` into shared functions callable by both daemon and MCP
- MCP tool registration matching current tool definitions (names, schemas, descriptions)
- Shared `nv-tools-core` module or direct nv-core dependency for types
- Configuration: read same `nv.toml` + Doppler secrets as daemon
- One mega-server with all tools (no domain splitting)

**Out of scope:**
- Removing tools from nv-daemon (dual-mode: daemon uses internally, MCP exposes externally)
- Telegram/channel changes (Phase 1/3 of cc-native)
- HTTP/SSE transport (stdio only for now)
- Domain-specific MCP servers (one binary handles all)
- CC Channels integration (blocked on research preview stability)
- Voice reply epic (nv-53k) -- defer to v7
- Dashboard changes
- Jira deferred tasks (retry, callbacks) -- not MCP-related

## Architecture Decision

```
crates/
  nv-core/       -- types, config (existing)
  nv-daemon/     -- daemon binary (existing, keeps tool dispatch)
  nv-cli/        -- CLI binary (existing)
  nv-tools/      -- NEW: MCP server binary
    src/
      main.rs    -- MCP server setup, stdio transport
      server.rs  -- tool registration, dispatch
      handlers/  -- tool handler functions (extracted from daemon)
```

### Extraction Strategy

Tools currently live in `crates/nv-daemon/src/tools/*.rs` as functions called by
`execute_tool()`. The extraction approach:

1. **Extract pure handler logic** from each tool file into functions that take typed inputs
   and return typed outputs (no daemon-specific state like SharedDeps)
2. **Create adapter layer** in nv-tools that maps MCP JSON inputs to typed handler calls
3. **Keep daemon's execute_tool()** calling the same handler functions (shared dependency)
4. **Secrets/config** loaded from same sources (Doppler + nv.toml)

### What stays in nv-daemon vs moves to shared

| Stays in nv-daemon | Moves to shared (nv-core or handler crate) |
|--------------------|--------------------------------------------|
| SharedDeps struct | Tool handler functions (pure logic) |
| execute_tool() dispatch | HTTP client construction |
| Orchestrator/worker | Config/secrets loading |
| Telegram/channels | Tool schemas/definitions |
| ConversationStore | |
| Health poller | |

## Priority Order

| Phase | Focus | Success Criteria |
|-------|-------|-----------------|
| 1 | MCP scaffold | `nv-tools` crate compiles, stdio MCP server starts, lists tools |
| 2 | Tool extraction | Handler functions extracted, callable from both daemon and MCP |
| 3 | Full tool coverage | All 60+ tools work via MCP with same behavior as daemon |
| 4 | Integration test | Claude Code can use nv-tools as MCP server for real queries |

## Success Gates (v6 is "done" when)

1. `nv-tools` binary starts and responds to MCP `tools/list` with all 60+ tools
2. `claude --mcp nv-tools` can execute at least 5 representative tools (jira_search,
   docker_status, ha_states, sentry_issues, read_memory)
3. nv-daemon still works unchanged (dual-mode, no regression)
4. Existing 1,032 tests still pass

## Hard Constraints

- Workspace crate in existing repo (not standalone)
- stdio transport only (no HTTP/SSE)
- One mega-server (no domain splitting)
- Must not break existing daemon functionality
- Same config/secrets sources (nv.toml + Doppler)
- Rust only (no TypeScript MCP SDK)

## Planning Model

Focused extraction work. Specs should be small and incremental:
1. Scaffold crate + MCP server skeleton
2. Extract tool handlers by domain (jira, sentry, ha, etc.)
3. Wire full dispatch
4. Integration test

## Timeline

No external deadline. Self-paced. Estimated 3-5 specs.
