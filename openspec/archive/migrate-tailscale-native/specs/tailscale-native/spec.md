# Capability: Tailscale Native

## ADDED Requirements

### Requirement: Native Tailscale service
The homelab host MUST run tailscaled natively via systemd instead of via Docker container.

#### Scenario: MagicDNS resolution
**Given** tailscaled runs natively on the host
**When** the daemon resolves hostname "homelab"
**Then** it returns the Tailscale IP 100.94.11.104

#### Scenario: Nexus connection
**Given** MagicDNS resolves homelab and macbook
**When** nv-daemon starts
**Then** Nexus agents connect successfully (no transport error)

## REMOVED Requirements

### Requirement: Docker Tailscale container
The tailscale Docker container SHALL be removed from compose/vpn.yml.

#### Scenario: Container no longer running
**Given** native tailscaled is active
**When** docker ps is checked
**Then** no tailscale container is running
