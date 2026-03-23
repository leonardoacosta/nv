# Capability: Doppler Secrets Management

## ADDED Requirements

### Requirement: Doppler project configuration
The NV repository SHALL declare its Doppler project and config via `doppler.yaml` at the repo root,
enabling `doppler run` to resolve secrets without explicit flags.

#### Scenario: Developer runs doppler from repo root
**Given** a developer is in the NV repo root
**When** they run `doppler run -- env | grep ANTHROPIC`
**Then** the `ANTHROPIC_API_KEY` from the `nova/prd` config is printed

#### Scenario: doppler.yaml declares project mapping
**Given** `doppler.yaml` exists at repo root
**When** parsed by the Doppler CLI
**Then** it maps to project `nova` and config `prd`

### Requirement: Systemd services use Doppler for secret injection
All three systemd services MUST use `doppler run --fallback=true --` to inject environment variables
instead of loading from a flat file.

#### Scenario: Daemon starts with Doppler secrets
**Given** `nv.service` is started by systemd
**When** the service starts
**Then** `ExecStart` runs `doppler run --fallback=true -- nv-daemon`
**And** all secrets are available as environment variables to the daemon process

#### Scenario: Daemon starts during Doppler outage
**Given** Doppler API is unreachable
**When** `nv.service` starts
**Then** `--fallback=true` uses locally cached secrets from the last successful fetch
**And** the daemon starts normally

#### Scenario: Relay services use Doppler
**Given** `nv-teams-relay.service` or `nv-discord-relay.service` is started
**When** the service starts
**Then** secrets are injected via `doppler run --fallback=true --`

### Requirement: CLI receives secrets via Doppler wrapper
The CLI MUST NOT parse `~/.nv/env` manually. Users SHALL invoke the CLI via
`doppler run -- nv <command>` to receive secrets.

#### Scenario: CLI invocation with Doppler
**Given** a user runs `doppler run -- nv ask "hello"`
**When** the CLI starts
**Then** all required secrets are present as environment variables
**And** no file I/O occurs to read `~/.nv/env`

## REMOVED Requirements

### Requirement: Flat file secret loading
The `~/.nv/env` flat file is no longer the source of truth for secrets. The CLI's manual env file
parser is removed.

#### Scenario: CLI without env file
**Given** `~/.nv/env` does not exist
**When** a user runs `doppler run -- nv status`
**Then** the CLI starts successfully (no file-not-found error or fallback logic)
