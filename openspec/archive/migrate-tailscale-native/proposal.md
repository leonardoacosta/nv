# Proposal: Migrate Tailscale Native

## Change ID
`migrate-tailscale-native`

## Summary
Move Tailscale from Docker container to native tailscaled on the host, enabling MagicDNS resolution for Nexus agent hostnames.

## Context
- Extends: homelab Docker compose (compose/vpn.yml), host systemd
- Related: Nexus DNS resolution failure explored in this session

## Motivation
Tailscale runs containerized with network_mode: host but MagicDNS names are unavailable to host processes. Nexus agents cannot resolve homelab/macbook hostnames. Native install fixes this with zero blast radius (container already uses host networking).

## Requirements

### Req-1: Native Tailscale installation
Install tailscale package natively on Arch Linux host, copy state from Docker volume, enable systemd service.

### Req-2: DNS verification
Verify homelab and macbook hostnames resolve via MagicDNS from the host.

### Req-3: Docker cleanup
Remove tailscale service from compose/vpn.yml. Persist sysctl ip_forward settings.

## Scope
- **IN**: Tailscale native install, state migration, Docker compose cleanup, sysctl persistence
- **OUT**: Changes to NV codebase, nv.toml changes, Nexus code changes

## Impact
| Area | Change |
|------|--------|
| Host system | Install tailscale package, enable tailscaled.service |
| ~/dev/hl/homelab/compose/vpn.yml | Remove tailscale service |
| /etc/sysctl.d/ | Add 99-tailscale.conf for ip_forward |

## Risks
| Risk | Mitigation |
|------|-----------|
| Brief network disruption during switchover | Stop container before starting native, state carries over |
