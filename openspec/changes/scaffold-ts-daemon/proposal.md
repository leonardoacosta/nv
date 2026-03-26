# Proposal: Scaffold TypeScript Daemon

## Change ID
`scaffold-ts-daemon`

## Summary

Create `packages/daemon/` — a new TypeScript project alongside the existing Rust codebase. This is the skeleton that all future TypeScript-based Nova subsystems build on. No business logic; foundation only.

## Context

- The existing Rust daemon lives in `crates/nv-daemon/`. The TypeScript daemon is a parallel, independent project.
- `packages/daemon/` will become the runtime target for specs that migrate or extend Nova's agent loop in TypeScript (Anthropic Agent SDK).
- Related archived specs: `replace-anthropic-with-agent-sdk`, `migrate-nova-brain`

## Motivation

The existing Rust daemon is functional but difficult to iterate on for AI orchestration logic. The TypeScript daemon gives us:

- Native Anthropic SDK + Agent SDK integration (no FFI, no proto bridge)
- Faster iteration on agent behavior (no recompile cycle)
- Full ecosystem of npm tools (pino logging, TOML parsing, etc.)
- A clean-room start: no legacy constraints from the Rust codebase

## Requirements

### Req-1: Package manifest

Create `packages/daemon/package.json` with:

- `name: "@nova/daemon"`, `version: "0.1.0"`, `type: "module"`
- `main: "dist/index.js"`, `types: "dist/index.d.ts"`
- Scripts: `dev` (tsx watch), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit)
- Runtime dependencies: `typescript`, `tsx`, `pino`, `@iarna/toml`, `dotenv`
- Dev dependencies: `@types/node`

### Req-2: TypeScript config

Create `packages/daemon/tsconfig.json`:

- `"strict": true`, `"target": "ES2022"`, `"module": "NodeNext"`, `"moduleResolution": "NodeNext"`
- `"outDir": "dist"`, `"rootDir": "src"`, `"declaration": true`, `"sourceMap": true`
- Path alias: `"@/*": ["src/*"]`

### Req-3: Entry point

Create `packages/daemon/src/index.ts`:

- Imports config, logger, and stub subsystem types
- Logs startup message via pino logger
- Exports a `main()` async function that loads config and prints it as a health check
- No actual service startup — placeholder comments for where channels, agent loop, etc. will be wired

### Req-4: Config loader

Create `packages/daemon/src/config.ts`:

- Reads `~/.nv/config/nv.toml` using `@iarna/toml`
- Falls back gracefully if file does not exist (returns defaults)
- Merges env vars (via `dotenv` for local dev, Doppler in production): `NV_LOG_LEVEL`, `NV_DAEMON_PORT`
- Exports `Config` type and `loadConfig(): Promise<Config>` function
- `Config` shape: `{ logLevel: string, daemonPort: number, configPath: string }`

### Req-5: Logger

Create `packages/daemon/src/logger.ts`:

- Wraps `pino` with project defaults: `level` from config/env, `transport.target: "pino-pretty"` in dev only
- Exports `createLogger(name: string): Logger` factory
- Exports a default `logger` instance for the root process

### Req-6: Core types

Create `packages/daemon/src/types.ts` with the following foundational types:

```typescript
export type Channel = "telegram" | "teams" | "discord" | "email" | "imessage";

export interface Message {
  id: string;
  channel: Channel;
  threadId?: string;
  senderId: string;
  senderName: string;
  content: string;
  receivedAt: Date;
}

export interface Trigger {
  id: string;
  pattern: string;         // regex or keyword
  channel?: Channel;       // null = all channels
  description: string;
}

export interface Obligation {
  id: string;
  description: string;
  sourceMessageId?: string;
  channel?: Channel;
  dueAt?: Date;
  createdAt: Date;
  status: "pending" | "in_progress" | "done" | "cancelled";
}
```

### Req-7: Workspace integration

Add `packages/daemon` to the workspace. Since this is a Rust workspace (`Cargo.toml`), no workspace config change is needed — `packages/daemon/` is a standalone Node project. Add a root-level `package.json` (if not present) or note that it is standalone.

## Scope

- **IN**: `packages/daemon/` directory, `package.json`, `tsconfig.json`, `src/index.ts`, `src/config.ts`, `src/logger.ts`, `src/types.ts`
- **OUT**: Any actual runtime logic, channel implementations, agent loop, HTTP server, tool registration, SQLite/DB setup

## Impact

| Area | Change |
|------|--------|
| `packages/daemon/package.json` | New file — project manifest |
| `packages/daemon/tsconfig.json` | New file — TS compiler config |
| `packages/daemon/src/index.ts` | New file — entry point |
| `packages/daemon/src/config.ts` | New file — TOML + env config loader |
| `packages/daemon/src/logger.ts` | New file — pino logger factory |
| `packages/daemon/src/types.ts` | New file — core domain types |

No changes to the Rust codebase. No changes to `Cargo.toml` or any existing file.

## Risks

| Risk | Mitigation |
|------|-----------|
| `@iarna/toml` ESM compatibility | Verify import at test time; fallback to `smol-toml` if needed |
| `pino-pretty` dev-only transport | Guard with `NODE_ENV !== "production"` check |
| `packages/` directory naming collides with future monorepo tooling | Use standalone `package.json` now; add workspace tooling (turbo/pnpm) in a separate spec |
| Path aliases (`@/*`) not resolving at runtime | Use `tsx` which handles path aliases natively; no `tsconfig-paths` needed |
