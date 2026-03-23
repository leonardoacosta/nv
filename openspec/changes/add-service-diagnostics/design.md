# Design: Service Diagnostics & Module Restructure

## Architecture

### Trait Hierarchy

```
Checkable (trait)
├── check_read() -> CheckResult       [required]
├── check_write() -> Option<CheckResult> [optional, default None]
└── name() -> &str                     [required]

ServiceRegistry<T: Checkable>
├── HashMap<String, T>                 [instance_name -> client]
├── project_map: HashMap<String, String> [project_code -> instance_name]
├── resolve(project) -> Option<&T>     [project_map → default_project → first]
├── get(instance) -> Option<&T>        [direct lookup]
├── default() -> Option<&T>            [single/first instance]
└── iter() -> impl Iterator            [for check_all enumeration]
```

### Check Flow

```
nv check (CLI)          check_services (tool)
     │                        │
     └──────┬─────────────────┘
            ▼
     check_all(registries)
            │
     ┌──────┴──────────────────────┐
     │  FuturesUnordered           │
     │  ┌─────────────────────┐    │
     │  │ stripe.check_read() │    │
     │  │ jira.check_read()   │    │
     │  │ sentry.check_read() │    │
     │  │ ...                 │    │
     │  └─────────────────────┘    │
     │           ▼                 │
     │  ┌─────────────────────┐    │
     │  │ stripe.check_write()│    │
     │  │ jira.check_write()  │    │
     │  │ ha.check_write()    │    │
     │  │ ...                 │    │
     │  └─────────────────────┘    │
     └─────────────────────────────┘
            │
            ▼
     CheckReport { channels, tools_read, tools_write, summary }
            │
     ┌──────┴──────┐
     │             │
   Terminal     JSON
   (colored)   (serde)
```

### Module Layout After Restructure

```
crates/nv-daemon/src/
├── main.rs
├── agent.rs
├── orchestrator.rs
├── worker.rs
├── callbacks.rs
├── health.rs
├── http.rs
├── memory.rs
├── messages.rs
├── conversation.rs
├── diary.rs
├── state.rs
├── bash.rs
├── claude.rs
├── tts.rs
├── voice_input.rs
├── speech_to_text.rs
├── account.rs
├── aggregation.rs
├── reminders.rs
├── scheduler.rs
├── shutdown.rs
├── tailscale.rs
├── channels/
│   ├── mod.rs          (re-exports, Channel trait)
│   ├── telegram/
│   ├── discord/
│   ├── teams/
│   ├── email/
│   └── imessage/
├── tools/
│   ├── mod.rs          (Checkable, ServiceRegistry, register_tools, execute_tool)
│   ├── check.rs        (CheckResult, CheckReport, check_all, format_terminal, format_json)
│   ├── jira/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── registry.rs
│   ├── stripe.rs
│   ├── vercel.rs
│   ├── sentry.rs
│   ├── neon.rs
│   ├── posthog.rs
│   ├── upstash.rs
│   ├── resend.rs
│   ├── ado.rs
│   ├── ha.rs
│   ├── docker.rs
│   ├── plaid.rs
│   ├── github.rs
│   ├── web.rs
│   ├── cloudflare.rs
│   ├── doppler.rs
│   ├── calendar.rs
│   └── schedule.rs
├── digest/
├── nexus/
└── query/
```

### Config Evolution

**Before (flat):**
```toml
[stripe]
# uses STRIPE_SECRET_KEY
```

**After (multi-instance, backward-compatible):**
```toml
# Option A: Flat (single instance) — no change needed
[stripe]
# uses STRIPE_SECRET_KEY

# Option B: Named instances
[stripe.instances.personal]
# uses STRIPE_SECRET_KEY_PERSONAL

[stripe.instances.llc]
# uses STRIPE_SECRET_KEY_LLC

[stripe.project_map]
OO = "personal"
CT = "llc"
```

### ServiceRegistry<T> Generic Pattern

```rust
pub struct ServiceRegistry<T: Checkable> {
    instances: HashMap<String, T>,
    project_map: HashMap<String, String>,
}

impl<T: Checkable> ServiceRegistry<T> {
    /// Resolve a client by project code.
    /// Chain: project_map -> default_project match -> first instance
    pub fn resolve(&self, project: &str) -> Option<&T> { ... }

    /// Direct instance lookup by name
    pub fn get(&self, instance: &str) -> Option<&T> { ... }

    /// Default/first instance (for services without project context)
    pub fn default(&self) -> Option<&T> { ... }

    /// Iterate all instances for check_all
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> { ... }
}
```

### Dry-Run Write Probe Strategy

| Service | Write Endpoint | Probe Payload | Expected Response |
|---------|---------------|---------------|-------------------|
| Jira | `POST /rest/api/3/issue` | `{"fields":{}}` | 400 "project is required" |
| Stripe | `POST /v1/invoices` | empty body | 400 "customer is required" |
| HA | `POST /api/services/light/turn_on` | `{}` | 400 or service-specific error |
| Vercel | `POST /v13/deployments` | `{}` | 400 validation error |
| Sentry | N/A (read-only tools) | — | `check_write()` returns None |
| Neon | N/A (read-only queries) | — | `check_write()` returns None |
| PostHog | N/A (read-only tools) | — | `check_write()` returns None |
| Resend | `POST /emails` | `{}` | 422 validation error |
| ADO | N/A (read-only tools) | — | `check_write()` returns None |
| Docker | N/A (read-only tools) | — | `check_write()` returns None |
| Plaid | N/A (read-only queries) | — | `check_write()` returns None |
| GitHub | N/A (read-only tools) | — | `check_write()` returns None |

### SharedDeps Evolution

```rust
// Before
pub struct SharedDeps {
    pub jira_registry: Option<JiraRegistry>,
    pub stripe_client: Option<StripeClient>,
    pub vercel_client: Option<VercelClient>,
    // ... 10 more Option<XClient> fields
}

// After
pub struct SharedDeps {
    pub jira: Option<ServiceRegistry<JiraClient>>,
    pub stripe: Option<ServiceRegistry<StripeClient>>,
    pub vercel: Option<ServiceRegistry<VercelClient>>,
    // ... all use ServiceRegistry<T>
}
```

### Risk: Restructure Diff Size

The module restructure touches every `use crate::` import for tools. To keep the diff reviewable:

1. **Batch 1**: Pure file moves + `mod` declaration changes. Zero logic changes.
2. **Batch 2**: `Checkable` trait + `ServiceRegistry<T>` + `check.rs` (new code only).
3. **Batch 3**: Multi-instance config expansion (config.rs changes).
4. **Batch 4**: `nv check` CLI + `check_services` tool (new code + wiring).

Each batch is independently compilable and testable.
