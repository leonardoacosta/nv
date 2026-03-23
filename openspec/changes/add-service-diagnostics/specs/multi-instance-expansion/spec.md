# Multi-Instance Expansion

## ADDED Requirements

### Requirement: Generic ServiceRegistry

The system MUST provide a generic `ServiceRegistry<T: Checkable>` that wraps a `HashMap<String, T>` with project-to-instance resolution. All service client fields in `SharedDeps` MUST use `ServiceRegistry<T>` instead of bare `Option<T>`.

#### Scenario: ServiceRegistry replaces per-service Option fields

**Given** `SharedDeps` currently has `Option<StripeClient>`, `Option<VercelClient>`, etc.
**When** the expansion is applied
**Then** each becomes `Option<ServiceRegistry<StripeClient>>` (or equivalent)
**And** `ServiceRegistry<T>` wraps `HashMap<String, T>` with:
  - `resolve(project: &str) -> Option<&T>` — lookup by project_map → default → first
  - `get(instance: &str) -> Option<&T>` — direct instance lookup
  - `iter() -> impl Iterator<Item = (&str, &T)>` — for check_all enumeration
  - `default() -> Option<&T>` — returns the single/default instance
**And** services that don't need multi-instance (Docker, GitHub) use a registry with one "default" entry

### Requirement: Generic Config Pattern

Every service config section MUST support both flat (single-instance, backward-compatible) and multi-instance TOML formats. Instance-qualified credential env vars MUST follow the `SERVICE_VAR_INSTANCENAME` naming convention with fallback to unqualified vars.

#### Scenario: All service configs support flat and multi-instance TOML

**Given** a service config section in `nv.toml`
**When** it has no `.instances` key
**Then** it is parsed as flat config (single instance, "default" name)
**When** it has `.instances.<name>` subsections
**Then** each is parsed as a named instance with separate credential env vars
**And** `project_map` optionally maps project codes to instance names

#### Scenario: Credential env var naming convention

**Given** a service `STRIPE` with instance name `personal`
**Then** the env var is `STRIPE_SECRET_KEY_PERSONAL` (service var + `_` + uppercase instance name)
**And** fallback to `STRIPE_SECRET_KEY` (unqualified) if instance-qualified var is missing
**And** this pattern applies uniformly to all services

### Requirement: Services Eligible for Multi-Instance

The system MUST support multi-instance configuration for all API-authenticated services (Stripe, Sentry, Vercel, PostHog, Neon, Upstash, Resend, ADO, HA, Plaid). Services using local-only auth (Docker, GitHub CLI) SHALL use a single "default" registry entry.

#### Scenario: Multi-instance capable services

**Given** the following services support multi-instance config:
  - Stripe (`STRIPE_SECRET_KEY_{INSTANCE}`)
  - Sentry (`SENTRY_AUTH_TOKEN_{INSTANCE}`, `SENTRY_ORG_{INSTANCE}`)
  - Vercel (`VERCEL_TOKEN_{INSTANCE}`)
  - PostHog (`POSTHOG_API_KEY_{INSTANCE}`, `POSTHOG_HOST_{INSTANCE}`)
  - Neon (`POSTGRES_URL_{INSTANCE}` — already instance-qualified by project code)
  - Upstash (`UPSTASH_REDIS_REST_URL_{INSTANCE}`, `UPSTASH_REDIS_REST_TOKEN_{INSTANCE}`)
  - Resend (`RESEND_API_KEY_{INSTANCE}`)
  - ADO (`ADO_PAT_{INSTANCE}`, `ADO_ORG_{INSTANCE}`)
  - HA (`HA_TOKEN_{INSTANCE}`, `HA_URL_{INSTANCE}`)
  - Plaid (`PLAID_DB_URL_{INSTANCE}`)

#### Scenario: Services that stay single-instance

**Given** Docker (local socket) and GitHub (`gh` CLI) have no multi-org use case
**Then** they use `ServiceRegistry` with one "default" entry
**And** their config sections remain flat-only
