# Implementation Tasks

<!-- beads:epic:nv-c7v9 -->

## Foundation Batch

- [ ] [1.1] Create `packages/daemon/package.json` — `name: "@nova/daemon"`, `type: "module"`, scripts: `dev` (tsx --watch src/index.ts), `build` (tsc), `start` (node dist/index.js), `typecheck` (tsc --noEmit); runtime deps: `typescript`, `tsx`, `pino`, `@iarna/toml`, `dotenv`; dev deps: `@types/node` [owner:api-engineer]
- [ ] [1.2] Create `packages/daemon/tsconfig.json` — `strict: true`, `target: ES2022`, `module: NodeNext`, `moduleResolution: NodeNext`, `outDir: dist`, `rootDir: src`, `declaration: true`, `sourceMap: true`, paths: `"@/*": ["src/*"]` [owner:api-engineer]
- [ ] [1.3] Create `packages/daemon/src/types.ts` — types: `Channel` (union), `Message`, `Trigger`, `Obligation` (with status union `"pending" | "in_progress" | "done" | "cancelled"`); all fields as documented in proposal Req-6 [owner:api-engineer]
- [ ] [1.4] Create `packages/daemon/src/config.ts` — `loadConfig(): Promise<Config>` reads `~/.nv/config/nv.toml` via `@iarna/toml`, falls back to defaults if missing, merges `NV_LOG_LEVEL` and `NV_DAEMON_PORT` env vars; exports `Config` type: `{ logLevel: string, daemonPort: number, configPath: string }` [owner:api-engineer]
- [ ] [1.5] Create `packages/daemon/src/logger.ts` — wraps `pino`; `createLogger(name: string): Logger` factory; default root `logger` export; pino-pretty transport enabled when `NODE_ENV !== "production"`; `level` sourced from `NV_LOG_LEVEL` env or `"info"` default [owner:api-engineer]
- [ ] [1.6] Create `packages/daemon/src/index.ts` — `main()` async entry: calls `loadConfig()`, creates logger, logs startup banner `{ service: "nova-daemon", version, configPath, daemonPort }`; placeholder comments for channels, agent loop, HTTP server wiring; calls `main()` at bottom [owner:api-engineer]

## Verify

- [ ] [2.1] `cd packages/daemon && npm install` (or `pnpm install`) completes without errors [owner:api-engineer]
- [ ] [2.2] `npm run typecheck` passes — zero TypeScript errors [owner:api-engineer]
- [ ] [2.3] `npm run dev` starts and prints startup banner to stdout [owner:api-engineer]
- [ ] [2.4] `npm run build` produces `dist/index.js` and type declarations [owner:api-engineer]
- [ ] [2.5] `node dist/index.js` runs and exits cleanly (no crash on missing toml file) [owner:api-engineer]
