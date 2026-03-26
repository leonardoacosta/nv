# Proposal: Add Messages Service

## Change ID
`add-messages-svc`

## Summary

Build the messages service (port 4002) as a Hono+MCP microservice that ports `get_recent_messages` and `search_messages` from the Rust daemon. Provides recent message retrieval with channel filtering and pgvector similarity search over 1536-dim embeddings.

## Context
- Roadmap: nova-v10, Wave 3 (Phase 2 — Core Tools)
- Depends on: `scaffold-tool-service` (template), `migrate-to-shared-postgres` (shared DB)
- Schema: `packages/db/src/schema/messages.ts` — uuid id, text channel, text sender, text content, jsonb metadata, timestamp created_at, vector embedding(1536)
- DB client: `packages/db/src/client.ts` — drizzle + postgres-js
- Rust reference: `crates/nv-daemon/src/worker.rs:2572-2624` — existing tool implementations
- Architecture: dual HTTP (Hono on :4002) + MCP (stdio) per scope-lock.md

## Motivation

The Rust daemon handles `get_recent_messages` and `search_messages` synchronously inside the worker because MessageStore wraps rusqlite::Connection (!Send). The TypeScript rewrite moved messages to Postgres but the tools are still embedded in the monolith daemon. Extracting them into an independent service gives:

1. **Process isolation** — message queries don't compete with agent dispatch or Telegram polling for event loop time
2. **Independent scaling** — can restart/update messages-svc without touching the daemon
3. **Dashboard access** — Traefik-exposed HTTP routes let the dashboard query messages directly instead of proxying through the daemon API
4. **MCP native discovery** — Agent SDK can discover message tools via stdio MCP without the tool-router intermediary

## Requirements

### Req-1: get_recent_messages Tool

Retrieve the most recent messages from Postgres, optionally filtered by channel. Returns messages ordered by `created_at` descending.

**Input:**
- `channel` (string, optional) — filter to a specific channel (telegram, discord, teams, etc.)
- `limit` (integer, optional, default: 20, max: 100) — number of messages to return

**Output:** Array of message objects with id, channel, sender, content, metadata, createdAt.

**HTTP:** `GET /recent?channel=telegram&limit=20`

#### Scenario: No filter

Given 50 messages exist across multiple channels,
when `get_recent_messages` is called with no arguments,
then it returns the 20 most recent messages across all channels, ordered newest first.

#### Scenario: Channel filter

Given 30 messages exist on the telegram channel and 20 on discord,
when `get_recent_messages` is called with `channel: "telegram"`,
then it returns only telegram messages, up to the limit.

#### Scenario: Empty result

Given no messages exist,
when `get_recent_messages` is called,
then it returns an empty array (not an error).

### Req-2: search_messages Tool

Search messages using pgvector cosine similarity on the embedding column. Falls back to ILIKE text search when no embedding is available for the query (embedding generation is out of scope — the caller provides the query vector or the service does text fallback).

**Input:**
- `query` (string, required) — search query text
- `channel` (string, optional) — filter results to a specific channel
- `limit` (integer, optional, default: 10, max: 50) — max results

**Output:** Array of matching message objects, ranked by relevance.

**HTTP:** `POST /search` with JSON body `{ query, channel?, limit? }`

#### Scenario: Text search fallback

Given messages exist containing "deploy failed on staging",
when `search_messages` is called with `query: "deploy staging"`,
then it returns messages where content matches via ILIKE, ordered by created_at desc.

#### Scenario: Channel-scoped search

Given matching messages exist on both telegram and discord,
when `search_messages` is called with `query: "meeting notes", channel: "teams"`,
then only teams channel messages are returned.

#### Scenario: No matches

Given no messages match the query,
when `search_messages` is called,
then it returns an empty array.

### Req-3: Health Endpoint

**HTTP:** `GET /health` returns `{ status: "ok", service: "messages-svc", uptime: <seconds> }`.

Used by tool-router health aggregation and systemd watchdog.

### Req-4: Dual HTTP + MCP Transport

The service runs as both:
- **HTTP server** (Hono on port 4002) — for dashboard and direct calls
- **MCP server** (stdio) — for Agent SDK native tool discovery

MCP exposes the same two tools (`get_recent_messages`, `search_messages`) with JSON Schema input definitions matching the HTTP API.

### Req-5: Service Structure

Follow the scaffold-tool-service template:
- `packages/tools/messages-svc/src/index.ts` — Hono app + server startup
- `packages/tools/messages-svc/src/tools.ts` — tool implementations (DB queries)
- `packages/tools/messages-svc/src/mcp.ts` — MCP server definition
- `packages/tools/messages-svc/package.json` — workspace package
- `packages/tools/messages-svc/tsconfig.json` — TypeScript config
- `packages/tools/messages-svc/build.mjs` — esbuild bundler

## Scope
- **IN**: get_recent_messages with channel filter, search_messages with text fallback, health endpoint, Hono HTTP routes, MCP stdio server, esbuild bundle, pino logging
- **OUT**: Embedding generation (messages arrive pre-embedded), pgvector similarity search (deferred until embedding pipeline ships), WebSocket streaming, authentication, rate limiting, message write/create endpoints, systemd unit file (handled by add-fleet-deploy), Traefik config (handled by add-fleet-deploy)

## Impact
| Area | Change |
|------|--------|
| `packages/tools/messages-svc/` | New: entire service directory |
| `packages/db/src/schema/messages.ts` | Read-only: consumed via import |
| `packages/db/src/client.ts` | Read-only: consumed via import |

## Risks
| Risk | Mitigation |
|------|-----------|
| pgvector extension not enabled on shared Postgres | migrate-to-shared-postgres spec handles extension creation; messages-svc text search works without it |
| Large message tables slow down unfiltered queries | Default limit of 20, max 100; index on created_at (already exists from schema) |
| MCP stdio conflicts with Hono server stdout | MCP uses stderr for logs, stdout exclusively for JSON-RPC; Hono server binds to TCP port, no stdout conflict |
