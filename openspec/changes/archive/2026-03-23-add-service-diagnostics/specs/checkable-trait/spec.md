# Checkable Trait & Check Infrastructure

## ADDED Requirements

### Requirement: Checkable Trait Definition

The system MUST define a `Checkable` async trait with `check_read()` returning `CheckResult` and an optional `check_write()` returning `Option<CheckResult>`. Each service client MUST implement this trait.

#### Scenario: Trait defines read and write health probes

**Given** a service implements `Checkable`
**When** `check_read()` is called
**Then** it performs a lightweight API call (GET or equivalent) and returns `CheckResult`
**And** when `check_write()` is called it performs a dry-run probe (expects 400/validation error) or returns `None` if no write endpoints exist

#### Scenario: CheckResult captures all possible states

**Given** a service check completes
**Then** the result is one of:
  - `Healthy { latency_ms: u64, detail: String }` — auth valid, endpoint reachable
  - `Degraded { latency_ms: u64, message: String }` — reachable but slow or partial
  - `Unhealthy { error: String }` — auth failed, endpoint down, or unexpected error
  - `Missing { env_var: String }` — required credential env var not set
  - `Disabled` — service disabled in config

### Requirement: Per-Service Checkable Implementations

Every service client MUST implement `Checkable` with a lightweight read probe appropriate to its auth pattern (Bearer, Basic, OAuth, connection string, or CLI). Services with write endpoints MUST implement `check_write()` using a dry-run probe that validates auth without creating resources.

#### Scenario: Bearer token services validate via lightweight GET

**Given** a service uses Bearer auth (Stripe, Vercel, Sentry, Resend, PostHog, Upstash)
**When** `check_read()` runs
**Then** it hits a cheap read endpoint (e.g., Stripe `GET /v1/balance`, Vercel `GET /v13/user`)
**And** returns `Healthy` with latency if 200, `Unhealthy` if 401/403

#### Scenario: Basic auth services validate via lightweight GET

**Given** a service uses Basic auth (Jira, ADO)
**When** `check_read()` runs
**Then** it hits a read endpoint (e.g., Jira `GET /rest/api/3/myself`, ADO `GET /_apis/projects`)
**And** returns `Healthy` with latency if 200, `Unhealthy` if 401

#### Scenario: Connection string services validate via connect

**Given** a service uses a connection string (Neon, Plaid)
**When** `check_read()` runs
**Then** it opens a connection and runs `SELECT 1`
**And** returns `Healthy` with latency, `Unhealthy` on connection failure

#### Scenario: CLI-based services validate via command

**Given** a service uses CLI auth (GitHub via `gh`, Docker via socket)
**When** `check_read()` runs
**Then** it runs a quick command (`gh auth status`, `docker info`)
**And** returns `Healthy` if exit code 0, `Unhealthy` otherwise

#### Scenario: OAuth services validate via token exchange

**Given** a service uses OAuth (Teams, Email via MS Graph)
**When** `check_read()` runs
**Then** it attempts a token refresh/acquire and hits `GET /me`
**And** returns `Healthy` if successful, `Unhealthy` if client credentials are invalid

#### Scenario: Write probes use dry-run requests

**Given** a service has write endpoints (Jira create, Stripe invoice, HA service_call)
**When** `check_write()` runs
**Then** it sends a request with intentionally invalid/empty body
**And** expects a 400-level validation error (proving auth is valid but input rejected)
**And** returns `Healthy` if the error is a validation error, `Unhealthy` if 401/403

### Requirement: check_all Orchestrator

The system MUST provide a `check_all()` function that runs all service probes concurrently and returns a structured `CheckReport` with per-service results grouped by category (channels, tools read, tools write).

#### Scenario: All services checked concurrently

**Given** `check_all()` is called with the full service list
**When** probes execute
**Then** all read checks run concurrently (tokio::join or FuturesUnordered)
**And** write checks run concurrently after read checks
**And** results are collected into a `Vec<(String, CheckResult, Option<CheckResult>)>` (name, read, write)
**And** total wall-clock time is bounded by the slowest single check + timeout
