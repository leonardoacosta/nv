# Tasks: fix-websocket-integration

**Spec:** fix-websocket-integration
**Status:** pending
**Beads Epic:** nv-my99

---

## Task List

### Batch 1: Dependencies and Config

- [ ] **T1** — Add `http-proxy` and `@types/http-proxy` to `apps/dashboard/package.json` devDependencies, and update the `start` script from `node .next/standalone/server.js` (or equivalent) to `node server.js`

### Batch 2: Custom Server

- [ ] **T2** — Create `apps/dashboard/server.ts` — custom Node.js HTTP server that:
  - Initializes the Next.js app with `next({ dev: process.env.NODE_ENV !== 'production' })`
  - Creates an HTTP server that passes all requests to the Next.js handler
  - Intercepts `upgrade` events: if `req.url === '/ws/events'`, proxies the upgrade to `${DAEMON_WS_URL}/ws/events` using `http-proxy`; otherwise destroys the socket
  - Derives `DAEMON_WS_URL` from `process.env.DAEMON_URL` by replacing `http://` with `ws://` and `https://` with `wss://`
  - Falls back to `ws://127.0.0.1:3443` when `DAEMON_URL` is not set
  - Listens on `PORT` env var, defaulting to `3000`
  - Logs startup: `console.log("server listening on port ${port}")`

### Batch 3: Dockerfile

- [ ] **T3** — Update `apps/dashboard/Dockerfile` CMD to run the custom server:
  - Add a build step that compiles `server.ts` to `server.js` alongside the Next.js standalone build
  - Update the final `CMD` from the current default standalone invocation to `["node", "server.js"]`
  - Ensure `server.js` and `node_modules/http-proxy` are present in the final image layer
  - Note: `output: "standalone"` in `next.config.ts` copies a minimal `node_modules` — verify `http-proxy` is included or copy it explicitly

### Batch 4: Validation

- [ ] **T4** — Verify locally: run `pnpm build && node server.js` in `apps/dashboard`, confirm `curl -i --include --no-buffer -H "Connection: Upgrade" -H "Upgrade: websocket" -H "Sec-WebSocket-Key: test" -H "Sec-WebSocket-Version: 13" http://localhost:3000/ws/events` returns `101 Switching Protocols` (when daemon is running)
- [ ] **T5** — Verify HTTP regression: confirm `curl http://localhost:3000/api/server-health` still returns daemon data (Next.js rewrites still work)
- [ ] **T6** — Docker smoke: rebuild the Docker image with `docker compose build dashboard` and run `docker compose up dashboard`, confirm the container starts and the WS proxy works

---

## Notes

- `DaemonEventContext.tsx`, `layout.tsx`, `Sidebar.tsx`, and all page files are already correct
  and require no modifications. The fix is purely in the server infrastructure layer.
- The `NEXT_PUBLIC_DAEMON_WS_HOST` env var in `DaemonEventContext.tsx` line 90 can remain as an
  override escape hatch — it only activates when explicitly set, which it never is in production.
- If `http-proxy` is too heavyweight, `ws` (npm package) or a manual `http.request` upgrade
  forward are acceptable alternatives. The engineer should pick the simplest option that works.
- For the Dockerfile, if `output: "standalone"` does not include `http-proxy` in the pruned
  `node_modules`, the engineer must either: (a) copy `http-proxy` explicitly in the Dockerfile,
  or (b) switch the approach to use Node's built-in `net` module for the raw TCP pipe to avoid
  the dependency entirely.
