# Implementation Tasks

<!-- beads:epic:nv-ecnr -->

## DB Batch

- [ ] [1.1] [P-1] Create packages/db/src/schema/auth.ts -- generate Better Auth schema tables (user, session, account, verification, apikey) using `npx @better-auth/cli generate` with Drizzle output, adjust table names if collision with existing `sessions` table (use `auth_session` or keep singular `session`) [owner:db-engineer] [beads:nv-wx83]
- [ ] [1.2] [P-2] Register auth schema in packages/db/src/client.ts -- import auth tables from schema/auth.ts and add to schema object so db.query.user, db.query.account etc. are available [owner:db-engineer] [beads:nv-qe1e]
- [ ] [1.3] [P-2] Export auth schema and types from packages/db/src/index.ts -- follow existing pattern: export tables + inferred select/insert types (User, NewUser, AuthSession, NewAuthSession, etc.) [owner:db-engineer] [beads:nv-mxan]
- [ ] [1.4] [P-2] Generate Drizzle migration -- run `pnpm db:generate` from packages/db to create SQL migration for new auth tables [owner:db-engineer] [beads:nv-jg80]

## API Batch

- [ ] [2.1] [P-1] Create packages/auth/ workspace package -- package.json with name "@nova/auth", deps: better-auth, @better-auth/api-key, @nova/db (workspace:*), tsconfig.json, src/index.ts with betterAuth() config using drizzleAdapter(db, { provider: "pg", schema }), emailAndPassword enabled, bearer() and apiKey() plugins [owner:api-engineer] [beads:nv-ws86]
- [ ] [2.2] [P-1] Create packages/auth/src/client.ts -- export authClient via createAuthClient() from better-auth/react, include apiKeyClient() plugin, export signIn, signUp, signOut, useSession convenience methods and Session/User types [owner:api-engineer] [beads:nv-afrs]
- [ ] [2.3] [P-2] Create packages/auth/src/seed.ts -- idempotent script: create admin user (NOVA_ADMIN_EMAIL / NOVA_ADMIN_PASSWORD env vars), generate API key via auth API, print key to stdout. Add `seed` script to package.json [owner:api-engineer] [beads:nv-bo7k]
- [ ] [2.4] [P-2] Create apps/dashboard/app/api/auth/[...all]/route.ts -- import auth from @nova/auth, export GET and POST via toNextJsHandler(auth) [owner:api-engineer] [beads:nv-bcul]
- [ ] [2.5] [P-2] Remove apps/dashboard/app/api/auth/verify/route.ts -- replaced by Better Auth sign-in endpoint [owner:api-engineer] [beads:nv-jsbg]
- [ ] [2.6] [P-2] Remove apps/dashboard/app/api/auth/logout/route.ts -- replaced by Better Auth sign-out endpoint [owner:api-engineer] [beads:nv-knmc]
- [ ] [2.7] [P-2] Add @nova/auth dependency to apps/dashboard/package.json [owner:api-engineer] [beads:nv-ve69]

## UI Batch

- [ ] [3.1] [P-1] Rewrite apps/dashboard/middleware.ts -- replace bearer token check with getSessionCookie() from better-auth/cookies, pass through /api/auth/* routes, keep CORS handling, keep dev-mode fallback (check BETTER_AUTH_SECRET instead of DASHBOARD_TOKEN), add legacy DASHBOARD_TOKEN fallback for migration period [owner:ui-engineer] [beads:nv-0aq7]
- [ ] [3.2] [P-1] Rewrite apps/dashboard/app/login/page.tsx -- replace token input with email/password form using authClient.signIn.email(), add sign-up toggle with name field using authClient.signUp.email(), preserve NovaMark branding and dark theme styling [owner:ui-engineer] [beads:nv-1y7j]
- [ ] [3.3] [P-1] Rewrite apps/dashboard/lib/auth.ts -- re-export auth server instance and helpers from @nova/auth instead of custom token utils [owner:ui-engineer] [beads:nv-i9to]
- [ ] [3.4] [P-1] Simplify apps/dashboard/lib/api-client.ts -- remove cookie parsing and Bearer header injection (session cookie sent automatically), keep only 401-redirect-to-login behavior [owner:ui-engineer] [beads:nv-3j1u]
- [ ] [3.5] [P-2] Update apps/dashboard/components/providers/DaemonEventContext.tsx -- remove ?token= query parameter from WebSocket URL (session cookie authenticates upgrade automatically) [owner:ui-engineer] [beads:nv-bom8]
- [ ] [3.6] [P-2] Update apps/dashboard/server.ts -- replace token query param validation on WS upgrade with auth.api.getSession({ headers: req.headers }) check, accept API key in Authorization header as fallback [owner:ui-engineer] [beads:nv-bbn5]
- [ ] [3.7] [P-2] Update apps/dashboard/components/Sidebar.tsx -- replace POST /api/auth/logout with authClient.signOut(), redirect to /login on success [owner:ui-engineer] [beads:nv-r02l]

## E2E Batch

- [ ] [4.1] Build verification -- run `pnpm build` from apps/dashboard to confirm no type errors or build failures after all changes [owner:ui-engineer] [beads:nv-9ol2]
- [ ] [4.2] Manual test -- start dashboard with BETTER_AUTH_SECRET set, verify /login shows email/password form, sign up creates user, sign in establishes session, all pages accessible [owner:user] [beads:nv-mwwu]
- [ ] [4.3] Manual test -- verify WebSocket connects with session cookie (no ?token= in URL), events stream normally [owner:user] [beads:nv-4at8]
- [ ] [4.4] Manual test -- verify logout clears session, redirects to /login, subsequent page requests redirect to /login [owner:user] [beads:nv-mxuy]
- [ ] [4.5] Manual test -- run seed script, verify API key authenticates requests via Authorization: Bearer header [owner:user] [beads:nv-slpe]
- [ ] [4.6] Manual test -- verify dev mode (no BETTER_AUTH_SECRET) allows all requests without auth [owner:user] [beads:nv-834r]
