# User Stories — NV (Master Agent Harness)

## Personas

NV is single-user (Leo only), but Leo interacts with NV in 3 distinct modes:

### 1. Leo as Commander

| Field | Value |
|-------|-------|
| **Role** | Solo operator issuing direct commands |
| **Goals** | Create Jira issues from phone, transition task status, trigger scans on demand |
| **Pain Points** | Opening Jira app is slow; context-switching from mobile to laptop to manage tasks; forgetting to create issues for things discussed in chat |
| **Technical Comfort** | Power user |

**Key interaction:** Leo messages NV on Telegram → NV understands intent → drafts action → Leo taps confirm.

### 2. Leo as Consumer

| Field | Value |
|-------|-------|
| **Role** | Passive recipient of proactive intelligence |
| **Goals** | Stay informed without checking 6 different apps; know what needs attention before it becomes urgent; morning briefing of overnight activity |
| **Pain Points** | Missing important messages across Discord/Teams/email; tasks falling through cracks; no unified view of "what happened while I was away" |
| **Technical Comfort** | Power user |

**Key interaction:** NV sends periodic digest → Leo reads → taps to approve/dismiss suggested actions.

### 3. Leo as Querier

| Field | Value |
|-------|-------|
| **Role** | Analyst asking questions about state across systems |
| **Goals** | "What's the status of X?" "Who mentioned Y?" "What sessions are running?" "What's blocking the OO release?" |
| **Pain Points** | Information scattered across Jira, Discord, Teams, git, Nexus sessions; no single place to ask cross-system questions |
| **Technical Comfort** | Power user |

**Key interaction:** Leo asks question on Telegram → NV queries memory + Jira + Nexus → responds with synthesized answer.

---

## User Flows

### Flow 1: Proactive Digest Cycle (Consumer)

The core loop — NV's reason for existence.

```mermaid
sequenceDiagram
    participant Cron as NV Cron Scheduler
    participant Agent as NV Agent Loop
    participant Memory as NV Memory
    participant Jira as Jira REST API
    participant Nexus as Nexus gRPC
    participant TG as Telegram Bot API
    participant Leo

    Cron->>Agent: Trigger digest (every 60min)

    par Gather context
        Agent->>Memory: Read recent conversation summaries
        Agent->>Jira: GET /rest/api/3/search (my open issues)
        Agent->>Nexus: GetSessions (all agents)
    end

    Agent->>Agent: Claude API — synthesize digest
    Note over Agent: "3 Jira issues need attention,<br/>2 sessions running on homelab,<br/>Teams thread about auth needs response"

    Agent->>TG: sendMessage (digest + inline keyboard)
    TG-->>Leo: 📋 Digest notification

    alt Leo taps "Create Jira for auth discussion"
        Leo->>TG: callback_query (confirm action)
        TG->>Agent: Confirmed action
        Agent->>Jira: POST /rest/api/3/issue (create)
        Agent->>TG: sendMessage ("Created OO-142: Auth discussion follow-up")
        Agent->>Memory: Append to tasks.md
    else Leo taps "Dismiss"
        Leo->>TG: callback_query (dismiss)
        TG->>Agent: Dismissed
        Agent->>Memory: Note dismissal for future triage
    end
```

### Flow 2: Direct Command (Commander)

Leo tells NV to do something specific.

```mermaid
sequenceDiagram
    participant Leo
    participant TG as Telegram Bot API
    participant Agent as NV Agent Loop
    participant Claude as Claude API
    participant Jira as Jira REST API
    participant Memory as NV Memory

    Leo->>TG: "Create a P1 bug for the checkout crash on OO"
    TG->>Agent: InboundMessage (telegram, Leo, text)

    Agent->>Memory: Read context (OO project, recent tasks)
    Agent->>Claude: Interpret command + draft Jira issue
    Claude-->>Agent: Structured response (title, description, priority, project)

    Agent->>TG: sendMessage ("Draft issue:<br/>Title: Checkout crash on payment flow<br/>Project: OO | Priority: P1 | Type: Bug<br/><br/>✅ Create  ❌ Cancel  ✏️ Edit")

    alt Leo taps ✅ Create
        Leo->>TG: callback_query (confirm)
        TG->>Agent: Confirmed
        Agent->>Jira: POST /rest/api/3/issue
        Jira-->>Agent: 201 Created (OO-143)
        Agent->>TG: sendMessage ("✅ Created OO-143")
        Agent->>Memory: Append to tasks.md
    else Leo taps ✏️ Edit
        Leo->>TG: callback_query (edit)
        Agent->>TG: sendMessage ("What would you like to change?")
        Leo->>TG: "Make it P0 and assign to me"
        TG->>Agent: Edit instruction
        Agent->>Claude: Revise draft
        Agent->>TG: sendMessage (revised draft + confirm buttons)
    end
```

### Flow 3: Context Query (Querier)

Leo asks a question that spans multiple systems.

```mermaid
sequenceDiagram
    participant Leo
    participant TG as Telegram Bot API
    participant Agent as NV Agent Loop
    participant Claude as Claude API
    participant Memory as NV Memory
    participant Jira as Jira REST API
    participant Nexus as Nexus gRPC

    Leo->>TG: "What's blocking the OO release?"
    TG->>Agent: InboundMessage (telegram, Leo, text)

    Agent->>Claude: Classify intent → context query

    par Gather data
        Agent->>Memory: Search for "OO release" context
        Agent->>Jira: GET /search?jql=project=OO AND status!=Done
        Agent->>Nexus: GetSessions (filter: project=oo)
    end

    Agent->>Claude: Synthesize answer from all sources
    Claude-->>Agent: Structured answer

    Agent->>TG: sendMessage (formatted answer)
    Note over TG,Leo: "OO Release Status:<br/>• 4 open issues (2 bugs, 2 tasks)<br/>• OO-139 (P0 bug) — checkout crash, unassigned<br/>• OO-141 (task) — migration pending review<br/>• 1 session on homelab applying 'add-webhooks' spec<br/>• Last Teams mention: Maria asked about timeline yesterday"

    Leo->>TG: "Assign OO-139 to me and move to In Progress"
    TG->>Agent: Follow-up command
    Agent->>TG: sendMessage ("Draft: Assign OO-139 to Leo, transition to In Progress. ✅ Confirm?")
    Leo->>TG: callback_query (confirm)
    Agent->>Jira: PUT /issue/OO-139 (assignee + transition)
    Agent->>TG: sendMessage ("✅ OO-139 assigned and moved to In Progress")
```

### Flow 4: Channel Message Triage (Consumer — future channels)

How NV handles inbound messages from monitored channels (post-MVP, but defines the architecture).

```mermaid
sequenceDiagram
    participant Source as Discord / Teams / Email
    participant Channel as NV Channel Adapter
    participant Gateway as NV Gateway
    participant Agent as NV Agent Loop
    participant Claude as Claude API
    participant Memory as NV Memory
    participant TG as Telegram → Leo

    Source->>Channel: New message in monitored channel
    Channel->>Gateway: InboundMessage (normalized)
    Gateway->>Agent: Route to agent loop

    Agent->>Memory: Read context (who is sender, what project, recent history)
    Agent->>Claude: Triage — is this actionable?

    alt Actionable (task, question, blocker)
        Claude-->>Agent: actionable=true, suggested_action
        Agent->>Memory: Store conversation summary
        Agent->>TG: sendMessage ("📩 Teams — Maria (OO):<br/>'When is the checkout fix shipping?'<br/><br/>Suggested: Reply with ETA from OO-139<br/>✅ Reply  📝 Draft  ❌ Skip")
    else Informational (FYI, no action needed)
        Claude-->>Agent: actionable=false, summary
        Agent->>Memory: Store summary for digest
        Note over Agent: Batched into next hourly digest
    else Noise (off-topic, automated, etc.)
        Claude-->>Agent: actionable=false, noise=true
        Note over Agent: Dropped, not stored
    end
```

### Flow 5: Session Awareness (Consumer + Querier)

NV uses Nexus to know what's happening in the dev environment.

```mermaid
sequenceDiagram
    participant Nexus as Nexus Agent (gRPC :7400)
    participant NV as NV Daemon
    participant Memory as NV Memory
    participant TG as Telegram → Leo

    NV->>Nexus: StreamEvents (subscribe)

    loop Continuous event stream
        Nexus-->>NV: SessionStarted (oo, /apply add-webhooks)
        NV->>Memory: Update sessions context

        Nexus-->>NV: StatusChanged (oo session → Idle)
        NV->>Memory: Update sessions context

        Nexus-->>NV: SessionStopped (oo, completed)
        NV->>Memory: Update sessions context
        NV->>TG: sendMessage ("🟢 Session completed: oo /apply add-webhooks (12min, $0.42)")
    end

    Note over NV,TG: Session data feeds into digests and query responses
```

---

## Interaction Surface Inventory

NV has no web UI. The interaction surfaces are:

| Surface | Type | Pages/Screens |
|---------|------|---------------|
| **Telegram** | Primary interface | Conversation flows (see wireframes) |
| **nv-cli** | Developer tool | `nv status`, `nv ask`, `nv config` |
| **systemd** | Service management | `systemctl status nv` |
| **TTS** | Audio notification | Via claude-daemon HTTP :9999 |

### Telegram Conversation "Pages"

| "Page" | Trigger | Content |
|--------|---------|---------|
| **Digest** | Cron (hourly) | Summary card + action buttons |
| **Command response** | Leo's message | Draft + confirm/cancel/edit buttons |
| **Query response** | Leo's question | Formatted answer with follow-up affordance |
| **Alert** | Real-time event | Session complete, urgent Jira change, etc. |
| **Confirmation** | After action | "✅ Done" receipt with link |

### CLI "Pages"

| Command | Output |
|---------|--------|
| `nv status` | Daemon health, connected channels, last digest, pending actions |
| `nv ask "question"` | Same as Telegram query, but in terminal |
| `nv config` | Show/edit nv.toml |
| `nv digest --now` | Trigger immediate digest |
