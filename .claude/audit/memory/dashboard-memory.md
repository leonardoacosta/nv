# Dashboard Domain Audit Memory

**Audited:** 2026-03-23  
**Auditor:** codebase-health-analyst  
**Scope:** `crates/nv-daemon/src/dashboard.rs`, `dashboard/src/`

## Health Scores

| Axis         | Score | Grade |
|--------------|-------|-------|
| Structure    | 72    | C     |
| Quality      | 55    | D     |
| Architecture | 68    | C     |
| **Health**   | **64**| **C** |

Composite: `(72 * 0.30) + (55 * 0.35) + (68 * 0.35) = 21.6 + 19.25 + 23.8 = 64.6`

## Critical Findings (Blocking)

### API/Frontend Contract Mismatches — Multiple Pages Broken

Five distinct request/response contract violations cause broken UI:

1. **PUT /api/memory** — frontend sends `{ path, content }`, backend expects `{ topic, content }`. Memory saves silently fail with 400. (`MemoryPage.tsx:59`, `dashboard.rs:566`)

2. **GET /api/memory** response shape — backend returns `{ topics: string[] }`, frontend expects `MemoryFile[]` or `{ files: MemoryFile[] }`. Falls through to `Object.entries()` path, producing a single synthetic "topics" entry. Memory page shows wrong data. (`MemoryPage.tsx:27`)

3. **PUT /api/config** — frontend sends `config` directly (`SettingsPage.tsx:213`), backend expects `{ fields: {...} }`. Settings save is broken (400 response). (`SettingsPage.tsx:213`, `dashboard.rs:675`)

4. **GET /api/obligations + /api/projects** — DashboardPage casts responses as raw arrays but APIs return `{ obligations: [...] }` / `{ projects: [...] }`. Dashboard always shows 0 for both counts. (`DashboardPage.tsx:82-90`)

5. **GET /api/sessions** — DashboardPage casts as `Session[]` but API returns `{ sessions: [...], ... }`. Dashboard sessions always empty. (`DashboardPage.tsx:93`)

### Secret Redaction Missing in GET /api/config

`config_json` is built by reading `config.toml` wholesale and converting to JSON with no field filtering (`main.rs:808-818`). If the config file contains secrets (API keys, bot tokens) in top-level scalar fields, they are served verbatim. The comment says "secrets redacted" but no redaction code exists.

**Note:** Secrets may be in env vars (loaded via `Secrets::from_env()`) and not in `config.toml`. If `config.toml` only stores non-secret config, this may not be exploitable in practice. Needs verification of what fields config.toml actually holds.

## Medium Findings

- **POST /api/solve** falls back to `/tmp` as cwd for unknown project codes instead of returning 400. (`dashboard.rs:461`)
- **HealthMetrics type mismatch** — NexusPage ServerHealth component expects flat `{ cpu_percent, memory_used_mb, ... }` but `/api/server-health` returns `{ daemon, latest, status, history }`. All metrics show as 0/undefined.
- **GET /api/memory** never populates `size_bytes` or `updated_at` on listing — sidebar metadata always empty.

## Low Findings

- No CORS middleware registered (intentional for homelab, undocumented)
- No CSP headers on any response
- No authentication on any endpoint (intentional for local daemon, undocumented)
- ObligationItem, ServerHealth, UsagePage use hardcoded hex colors instead of theme tokens
- UsagePage pulls tool stats from `/api/sessions` which never includes `tool_stats` — should use `/stats`
- PATCH `/api/obligations/{id}` acquires the mutex 3 times (verify/update/re-fetch)

## Architecture Notes

- SPA is embedded at compile time via `rust-embed` — correct pattern, no runtime file serving risk
- Asset cache headers are correct: `no-cache` for index.html, `immutable` for hashed assets
- Path traversal on PUT /api/memory is properly blocked via character whitelist in `put_memory` handler (`dashboard.rs:592-603`)
- Path traversal on GET /api/memory (topic read) is blocked via `sanitize_topic()` in `memory.rs:613`
- No TOML injection risk on PUT /api/config — key allowlist (existing top-level scalars only) and `json_to_toml` type mapping prevent injection

## Recommended Next Actions (Priority Order)

1. Fix all 5 API/frontend contract mismatches — these cause broken core functionality
2. Audit `config.toml` to verify no secrets are stored there; if they are, add explicit redaction before serving
3. Add `{ fields: {...} }` wrapper to SettingsPage save payload
4. Fix PUT /api/memory client to send `topic` not `path`
5. Fix GET /api/memory client to handle `{ topics: string[] }` and fetch individual file content on select
6. Fix NexusPage HealthMetrics type to match actual `/api/server-health` response shape
7. Replace hardcoded hex colors with theme tokens across ObligationItem, ServerHealth, UsagePage
