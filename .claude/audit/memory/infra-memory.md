# Infrastructure — Audit Memory

## Route Inventory
- GET /health (deep probe option)
- GET /stats (token counts, tool usage)

## Key Modules
- health.rs, health_poller.rs, server_health_store.rs — health system
- nv-core/config.rs — TOML config parsing
- nv-core/types.rs — shared types
- memory.rs — markdown memory (20K limit, auto-summarize @ 20 H2s)
- state.rs — JSON persistence (~/.nv/state/)
- messages.rs — SQLite (messages + obligations)
- nv-cli/ — status, ask, check, digest, stats commands
- deploy/ — systemd service, install script
- shutdown.rs — graceful shutdown

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
