# Implementation Tasks
<!-- beads:epic:nv-t48c -->

## API Batch

- [ ] [2.1] [P-1] Copy service-template to graph-svc, update package.json name to `@nova/graph-svc`, set SERVICE_NAME=graph-svc and SERVICE_PORT=4007 defaults in config.ts, add CLOUDPC_HOST and CLOUDPC_USER_PATH config entries [owner:api-engineer] [beads:nv-6dmo,nv-b0ej]
- [ ] [2.2] [P-1] Create src/ssh.ts -- SSH helper that spawns `ssh cloudpc "powershell ..."` via child_process.execFile with 10s connect timeout, 30s execution timeout, noise line filtering, and error classification (503 unreachable, 502 script error, 504 timeout) [owner:api-engineer] [beads:nv-uxdv,nv-vehs]
- [ ] [2.3] [P-1] Create src/tools/calendar.ts -- three handlers: calendar_today (runs graph-outlook.ps1 -Action CalendarToday), calendar_upcoming (runs graph-outlook.ps1 -Action CalendarUpcoming -Days N), calendar_next (runs graph-outlook.ps1 -Action CalendarNext) [owner:api-engineer] [beads:nv-ocrk]
- [ ] [2.4] [P-1] Create src/tools/ado.ts -- three handlers: ado_projects (runs graph-ado.ps1 -Action Projects), ado_pipelines (runs graph-ado.ps1 -Action Pipelines [-Project X]), ado_builds (runs graph-ado.ps1 -Action Builds [-Project X] [-Pipeline Y] [-Limit N]) [owner:api-engineer] [beads:nv-04a4]
- [ ] [2.5] [P-1] Create src/tools/index.ts -- register all 6 tools in the ToolRegistry with JSON Schema inputSchema definitions matching the Rust tool definitions pattern [owner:api-engineer] [beads:nv-7032]
- [ ] [2.6] [P-1] Wire Hono HTTP routes in src/http.ts -- GET /calendar/today, /calendar/upcoming, /calendar/next, /ado/projects, /ado/pipelines, /ado/builds with query param parsing and proper error status codes [owner:api-engineer] [beads:nv-s4pn]
