# Dashboard — Audit Memory

## Route Inventory
- GET /api/obligations (list with filters)
- PATCH /api/obligations/{id} (status update)
- GET /api/projects (project codes)
- GET /api/sessions (Nexus sessions)
- POST /api/solve (start Nexus session)
- GET /api/memory (topic listing/read)
- PUT /api/memory (file write)
- GET /api/config (redacted config)
- PUT /api/config (config update)
- GET /api/server-health (metrics)
- SPA: /, /obligations, /projects, /nexus, /integrations, /usage, /memory, /settings

## Key Components
- React + React Router + Vite + Tailwind (cosmic-gradient, dark mode)
- Sidebar, ServerHealth, UsageSparkline, ActiveSession, IntegrationCard, NovaMark

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
