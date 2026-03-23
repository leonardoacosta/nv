# User Stories — Nova v4

## Personas

Nova has one user (Leo) but three distinct interaction modes that create different UX needs.
Each persona represents Leo in a different context.

### Persona 1: Leo — Mobile Operator
- **Role**: DevOps lead checking in between meetings, on the go
- **Interface**: Telegram on iPhone
- **Goals**:
  - See what needs attention in <10 seconds (obligations, alerts, failures)
  - Approve or reject pending actions with one tap
  - Ask quick cross-system questions ("what's the status of OO deploy?")
- **Pain Points**:
  - Telegram output is sometimes too verbose for mobile
  - Confirmation buttons don't always surface properly
  - No way to see "what did I miss?" at a glance
- **Technical Comfort**: Power user

### Persona 2: Leo — Desktop Commander
- **Role**: Engineer at his desk, managing projects, reviewing code
- **Interface**: Web dashboard + CLI
- **Goals**:
  - See full system health across all 20+ projects on one screen
  - Drill into specific project health (deploys, errors, tickets)
  - Delegate code changes to Nexus agents and monitor progress
  - Review obligation queue and mark items as handled
- **Pain Points**:
  - Switching between Jira, Vercel, Sentry, GitHub, ADO tabs constantly
  - No unified view of "what's broken across all projects?"
  - Can't see Nova's tool usage, memory, or session history
- **Technical Comfort**: Power user

### Persona 3: Leo — Sleeping / AFK
- **Role**: Not at keyboard — Nova operates autonomously
- **Interface**: None (Nova acts proactively, queues results for review)
- **Goals**:
  - Nova watches deploys, error rates, and channels while Leo is away
  - Obligations detected from incoming messages are queued, not lost
  - P0 alerts still reach Leo via Telegram push notification
  - Non-urgent items accumulate in dashboard for morning review
- **Pain Points**:
  - Currently, obligations from overnight messages are lost (no detection)
  - Digest is sometimes empty even when things happened
  - No "morning briefing" that says "here's what you missed"
- **Technical Comfort**: N/A (autonomous operation)

---

## User Flows

### Flow 1: Proactive Obligation Detection (Must-Do #1)

The core v4 flow — Nova detects an obligation from an incoming message across any channel
and surfaces it to Leo without being asked.

```mermaid
sequenceDiagram
    participant Client as Client/Coworker
    participant Channel as Discord/Teams/Email
    participant Nova as Nova Daemon
    participant Claude as Claude (Analysis)
    participant Store as Obligation Store
    participant TG as Telegram
    participant Dash as Dashboard

    Client->>Channel: "Hey, can you update the landing page copy?"
    Channel->>Nova: InboundMessage (channel: discord)
    Nova->>Claude: Analyze: is this an obligation for Leo?
    Claude-->>Nova: Yes — action requested, owner: Leo, project: OO
    Nova->>Store: Save obligation (source, text, project, priority, deadline)
    Nova->>TG: 🔔 New obligation from Discord<br/>[OO] Update landing page copy<br/>Source: @client_name
    Nova->>Dash: Update obligation queue (WebSocket/SSE)
    Note over Nova: If quiet hours → queue only, no TG push
```

### Flow 2: Code-Aware Operations (Must-Do #2)

Leo asks Nova to investigate a codebase issue and delegate a fix to a Nexus agent.

```mermaid
sequenceDiagram
    participant Leo as Leo (Telegram)
    participant Nova as Nova Daemon
    participant Claude as Claude (Analysis)
    participant Nexus as Nexus Agent
    participant CC as Claude Code Session
    participant TG as Telegram

    Leo->>Nova: "The OO deploy is failing — can you look into it?"
    Nova->>Claude: Analyze request, gather context
    Claude->>Nova: tool_call: sentry_issues("oo"), vercel_deployments("oo")
    Nova-->>Claude: Sentry: 3 new errors, Vercel: last deploy ERRORED
    Claude->>Nova: tool_call: nexus_start_session(project: "oo", command: "/ci:gh --fix")
    Nova->>TG: ⏳ Pending: Start CC session on OO to fix CI<br/>[Approve] [Edit] [Cancel]
    Leo->>TG: [Approve]
    TG->>Nova: callback: approve:{uuid}
    Nova->>Nexus: StartSession(project: oo, command: /ci:gh --fix)
    Nexus->>CC: Spawn Claude Code session
    CC-->>Nexus: Session started: s-abc123
    Nexus-->>Nova: Session s-abc123 active
    Nova->>TG: ✅ Session started on OO (s-abc123)<br/>Monitoring...
    Note over Nova: Poll session status via Nexus
    Nexus-->>Nova: Session complete — CI passing
    Nova->>TG: ✅ OO CI fixed. Commit: abc1234
```

### Flow 3: Dashboard Morning Review (Desktop Commander)

Leo opens the dashboard to review overnight activity.

```mermaid
sequenceDiagram
    participant Leo as Leo (Browser)
    participant Dash as Dashboard SPA
    participant API as Nova API (:8400)
    participant DB as SQLite

    Leo->>Dash: Open http://homelab:8400/
    Dash->>API: GET /api/obligations?status=open
    Dash->>API: GET /api/stats
    Dash->>API: GET /api/health?deep=true
    API->>DB: Query obligations, messages, tool_usage
    DB-->>API: Results
    API-->>Dash: JSON responses
    Dash-->>Leo: Dashboard renders:<br/>• 3 open obligations<br/>• 2 deploy failures overnight<br/>• System health: 18/20 services OK

    Leo->>Dash: Click obligation: "Update landing page copy"
    Dash->>API: GET /api/obligations/{id}
    API-->>Dash: Full context (source message, channel, timestamp)
    Leo->>Dash: Mark as "Acknowledged" / "Delegated to Nexus"
    Dash->>API: PATCH /api/obligations/{id} {status: "acknowledged"}
```

### Flow 4: Deploy Failure Auto-Alert (Proactive, AFK)

Nova detects a deploy failure without being asked and alerts Leo.

```mermaid
sequenceDiagram
    participant Cron as Cron Trigger (every 5m)
    participant Nova as Nova Daemon
    participant Vercel as Vercel API
    participant Sentry as Sentry API
    participant Store as Alert Store
    participant TG as Telegram

    Cron->>Nova: trigger: deploy_watch
    Nova->>Vercel: vercel_deployments("oo")
    Vercel-->>Nova: Latest: ERROR (2min ago)
    Nova->>Sentry: sentry_issues("oo", since: 10m)
    Sentry-->>Nova: 5 new errors (TypeError in auth.ts)
    Nova->>Store: Save alert (type: deploy_failure, project: oo)
    Nova->>TG: 🔴 Deploy failed: OO<br/>Vercel: ERROR (2min ago)<br/>Sentry: 5 new errors (TypeError in auth.ts)<br/>[🔄 Retry Deploy] [🐛 Investigate]
    Note over Nova: If Leo taps Investigate → Flow 2
```

### Flow 5: HA Anomaly Detection (Proactive, Home)

Nova detects unusual Home Assistant state and alerts.

```mermaid
sequenceDiagram
    participant Cron as Cron Trigger (every 15m)
    participant Nova as Nova Daemon
    participant HA as Home Assistant
    participant TG as Telegram

    Cron->>Nova: trigger: ha_watch
    Nova->>HA: ha_states() — all entities
    HA-->>Nova: 312 entities
    Nova->>Nova: Compare against baseline<br/>Detect: garage door open > 30min
    Nova->>TG: ⚠️ HA Anomaly: Garage door open 47 minutes<br/>[Close Door] [Dismiss]
    Note over TG: [Close Door] triggers ha_service_call<br/>with PendingAction confirmation
```

---

## Page Inventory

Pages derived from user flows, mapped to wireframe files.

| Flow | Screen | Wireframe |
|------|--------|-----------|
| All | Navigation hub | `index.html` |
| Flow 3 | System overview | `pages/dashboard.html` |
| Flow 1,3 | Obligation queue | `pages/obligations.html` |
| Flow 2,4 | Project detail | `pages/project.html` |
| Flow 2 | Nexus sessions | `pages/sessions.html` |
| Flow 4,5 | Alerts & events | `pages/alerts.html` |
| All | Settings / config | `pages/settings.html` |
