# PRD -- Nova v9

> Clean-room TypeScript rewrite of Nova's daemon. Agent SDK + Vercel AI Gateway + Postgres/pgvector.

## Summary

Rewrite Nova's 102K-line Rust daemon in TypeScript, using the Anthropic Agent SDK as the
orchestration core. Every inbound message becomes an Agent SDK `query()` call with Nova's system
prompt, conversation history from Postgres, and tools accessible via Claude Code's built-in
Bash/Read/Write + documented CLI wrappers. Vercel AI Gateway provides observability. Claude MAX
subscription provides model access with no API key.

## Architecture

```
Telegram → Nova TS Daemon (Node.js, systemd)
             ├── Agent SDK query() per message
             │     ├── Claude MAX (OAuth, no API key)
             │     └── Vercel AI Gateway (observability)
             ├── CLI Tool Wrappers (Bash-invoked)
             │     ├── teams-cli, outlook-cli, ado-cli, az, discord-cli
             ├── Drizzle ORM → Postgres + pgvector (local Docker)
             └── HTTP API (Express/Hono for dashboard)
```

## Phases

| Phase | Label | Specs | Dependency |
|-------|-------|-------|-----------|
| 1 | Foundation | Postgres schema, Telegram adapter, Agent SDK integration, HTTP API, systemd | None |
| 2 | Tool Wrappers | teams-cli, outlook-cli, ado-cli, discord-cli, tool docs | Phase 1 |
| 3 | Features | Obligations, proactive watcher, diary, briefing, memory | Phase 1 |
| 4 | Dashboard Integration | API parity, WebSocket, obligation CRUD | Phase 1+3 |
